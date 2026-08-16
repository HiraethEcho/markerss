package ui

import (
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"strings"
	"testing"

	"markerss/internal/config"
	"markerss/internal/feedlist"
	"markerss/internal/store"
)

func newFeedServer(t *testing.T, rss string) *httptest.Server {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Write([]byte(rss))
	}))
	t.Cleanup(srv.Close)
	return srv
}

const bodyRSS = `<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0"><channel><title>B</title>
<item><title>With Body</title><link>http://x.test/1</link><guid>gb1</guid>
<description>&lt;p&gt;short summary text&lt;/p&gt;</description>
<content:encoded xmlns:content="http://purl.org/rss/1.0/modules/content/">&lt;p&gt;FULL BODY HTML here&lt;/p&gt;</content:encoded></item>
<item><title>No Body</title><link>http://x.test/2</link><guid>gb2</guid>
<description>&lt;p&gt;only summary&lt;/p&gt;</description></item>
</channel></rss>`

func newBodyModel(t *testing.T) *Model {
	srv := newFeedServer(t, bodyRSS)
	st, err := store.Open(filepath.Join(t.TempDir(), "m.db"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { st.Close() })
	m := New(Config{Store: st,
		Feeds:    []feedlist.Feed{{URL: srv.URL, Title: "B", Category: "tech"}},
		URLsPath: filepath.Join(t.TempDir(), "urls"),
		Cfg:      config.Default(""), DataDir: t.TempDir()})
	m.width, m.height = 120, 40
	return refresh(t, m)
}

func TestSummaryBodyModes(t *testing.T) {
	m := newBodyModel(t)
	// newest-first by rowid: gb2 "No Body" is items[0]
	m.navSel = m.findRow(func(r navRow) bool { return r.kind == rowSection && r.section == "Unread" })
	m.updateKeys(key("enter"))
	joined := strings.Join(m.artMD, "\n")
	if !strings.Contains(joined, "only summary") {
		t.Errorf("preview should show summary: %s", joined)
	}
	if strings.Contains(joined, "FULL BODY") {
		t.Errorf("preview must not show body: %s", joined)
	}
	// enter → opens "No Body" → no body → hint
	m.updateKeys(key("enter"))
	joined = strings.Join(m.artMD, "\n")
	if !strings.Contains(joined, "no body in feed") {
		t.Errorf("no-body item should show fetch hint: %s", joined)
	}
	// n → next item "With Body" → RSS body shows
	m.updateKeys(key("n"))
	joined = strings.Join(m.artMD, "\n")
	if !strings.Contains(joined, "FULL BODY") {
		t.Errorf("article mode must show RSS body: %s", joined)
	}
	if strings.Contains(joined, "only summary") {
		t.Errorf("body mode should not repeat summary text as body: %s", joined)
	}
}
