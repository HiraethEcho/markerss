CXX      ?= g++
CXXFLAGS ?= -std=c++20 -Wall -Wextra -O0 -g
LIBS     := -lncursesw

SRC      := src
BIN      := build

all: $(BIN)/markerss $(BIN)/test_feedlist

$(BIN):
	mkdir -p $(BIN)

$(BIN)/markerss: $(SRC)/main.cpp $(SRC)/feedlist.cpp $(SRC)/feedlist.h $(SRC)/xdg.cpp $(SRC)/xdg.h | $(BIN)
	$(CXX) $(CXXFLAGS) -o $@ $(SRC)/main.cpp $(SRC)/feedlist.cpp $(SRC)/xdg.cpp $(LIBS)

$(BIN)/test_feedlist: $(SRC)/test_feedlist.cpp $(SRC)/feedlist.cpp $(SRC)/feedlist.h | $(BIN)
	$(CXX) $(CXXFLAGS) -o $@ $(SRC)/test_feedlist.cpp $(SRC)/feedlist.cpp

test: $(BIN)/test_feedlist
	./$(BIN)/test_feedlist

run: $(BIN)/markerss
	./$(BIN)/markerss

clean:
	rm -rf $(BIN)

.PHONY: all test run clean
