# Cross-build CYD Companion for Windows (MinGW)
CXX := x86_64-w64-mingw32-g++
ROOT := $(abspath ..)
OUT := $(ROOT)/dist/cyd-companion-windows
EXE := $(OUT)/cyd-companion.exe

CXXFLAGS := -O2 -std=c++17 -Wall -Wextra -municode -DUNICODE -D_UNICODE -mwindows
LDFLAGS := -mwindows -municode -static -static-libgcc -static-libstdc++
LIBS := -lcomctl32 -lsetupapi -lws2_32 -lgdi32 -luser32 -lkernel32 -lcomdlg32 -lole32

SRCS := src/main.cpp src/serial_win.cpp src/protocol.cpp
OBJS := $(SRCS:src/%.cpp=build/%.o)

.PHONY: all clean dirs

all: dirs $(EXE)

dirs:
	mkdir -p build "$(OUT)"

build/%.o: src/%.cpp
	$(CXX) $(CXXFLAGS) -Isrc -c $< -o $@

$(EXE): $(OBJS)
	$(CXX) $(OBJS) -o $@ $(LDFLAGS) $(LIBS)

clean:
	rm -rf build
