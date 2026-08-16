// Package fetch pulls feeds (refresh, summary only) and full article
// pages (readability extraction, on explicit request).
package fetch

import (
	"context"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
	"time"

	"github.com/go-shiori/go-readability"
	"github.com/mmcdole/gofeed"

	"markerss/internal/store"
)

const maxBody = 8 << 20 // 8 MiB cap

// Options configures the fetch client.
type Options struct {
	TimeoutSec      int    // per-request timeout
	Proxy           string // HTTP proxy URL, empty = direct
	MaxItemsPerFeed int    // cap items kept per feed (0 = unlimited)
}

// Client performs feed and article fetches.
type Client struct {
	httpc *http.Client
	ua    string
	max   int
}

// New returns a Client with the given options.
func New(opts Options) *Client {
	if opts.TimeoutSec <= 0 {
		opts.TimeoutSec = 30
	}
	c := &Client{
		httpc: &http.Client{Timeout: time.Duration(opts.TimeoutSec) * time.Second},
		ua:    "markerss/0.1 TUI RSS reader",
		max:   opts.MaxItemsPerFeed,
	}
	if opts.Proxy != "" {
		if u, err := url.Parse(opts.Proxy); err == nil {
			c.httpc.Transport = &http.Transport{Proxy: http.ProxyURL(u)}
		}
	}
	return c
}

// Refresh fetches and parses one feed, returning its items. Summary
// only — full content is never fetched here.
func (c *Client) Refresh(feedURL string) ([]store.Item, error) {
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, feedURL, nil)
	if err != nil {
		return nil, err
	}
	req.Header.Set("User-Agent", c.ua)
	resp, err := c.httpc.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("%s: HTTP %d", feedURL, resp.StatusCode)
	}
	body, err := io.ReadAll(io.LimitReader(resp.Body, maxBody))
	if err != nil {
		return nil, err
	}
	feed, err := gofeed.NewParser().ParseString(string(body))
	if err != nil {
		return nil, fmt.Errorf("%s: %w", feedURL, err)
	}
	var items []store.Item
	for _, it := range feed.Items {
		if it == nil {
			continue
		}
		guid := strings.TrimSpace(it.GUID)
		if guid == "" {
			guid = strings.TrimSpace(it.Link)
		}
		if guid == "" {
			continue
		}
		items = append(items, store.Item{
			FeedURL:   feedURL,
			GUID:      guid,
			Title:     strings.TrimSpace(it.Title),
			Link:      strings.TrimSpace(it.Link),
			Published: published(it),
			Summary:   it.Description,
			Body:      it.Content,
		})
		if c.max > 0 && len(items) >= c.max {
			break
		}
	}
	return items, nil
}

func published(it *gofeed.Item) time.Time {
	if it.PublishedParsed != nil {
		return *it.PublishedParsed
	}
	if it.UpdatedParsed != nil {
		return *it.UpdatedParsed
	}
	return time.Time{}
}

// Article fetches a page and extracts its main content (readability).
// Returns the extracted article HTML; caller stores it.
func (c *Client) Article(pageURL string) (string, error) {
	ctx, cancel := context.WithTimeout(context.Background(), 60*time.Second)
	defer cancel()
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, pageURL, nil)
	if err != nil {
		return "", err
	}
	req.Header.Set("User-Agent", c.ua)
	resp, err := c.httpc.Do(req)
	if err != nil {
		return "", err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return "", fmt.Errorf("HTTP %d", resp.StatusCode)
	}
	u, err := url.Parse(pageURL)
	if err != nil {
		return "", err
	}
	art, err := readability.FromReader(resp.Body, u)
	if err != nil {
		return "", err
	}
	return art.Content, nil
}
