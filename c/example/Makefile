UNAME := $(shell uname)
LIBNOOSPHERE := ../../target/debug/deps/libnoosphere.a
HEADER := ../../target/headers/include/noosphere/noosphere.h
INCLUDE_PATH := ../../target/headers/include/noosphere

ifeq ($(UNAME), Darwin)
	LINKER_FLAGS := -framework Security
else
	LINKER_FLAGS :=
endif

$(HEADER):
	../../scripts/generate-headers.sh
$(LIBNOOSPHERE):
	cargo build -p noosphere
main.o: $(HEADER)
	$(CC) -I$(INCLUDE_PATH) -c main.c
main.out: main.o $(LIBNOOSPHERE)
	$(CC) main.o $(LINKER_FLAGS) $(LIBNOOSPHERE) -I$(INCLUDE_PATH) -lm -o main.out

.PHONY: build run clean

build: main.out

run: main.out
	./main.out
clean:
	rm -rf $(HEADER) main.o main.out
