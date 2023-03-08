LIBNOOSPHERE := ../../target/debug/deps/libnoosphere.a

noosphere.h:
	cargo run -p noosphere --example generate_header --features headers
$(LIBNOOSPHERE):
	cargo build -p noosphere
main.o: noosphere.h	
	$(CC) -c main.c
main.out: main.o $(LIBNOOSPHERE)
	$(CC) main.o $(LIBNOOSPHERE) -lm -o main.out

.PHONY: main run clean

main: main.out

run: main.out
	./main.out
clean:
	rm -rf noosphere.h main.o main.out
