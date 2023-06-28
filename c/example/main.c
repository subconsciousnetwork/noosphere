#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "noosphere.h"

#ifdef NDEBUG
#error Asserting asserts are asserted.
#endif

char *str_from_buffer(slice_boxed_uint8_t buffer) {
  char *message = (char *)malloc(buffer.len + 1);
  if (message) {
    strncpy(message, (char *)buffer.ptr, buffer.len);
    message[buffer.len] = '\0';
  }
  return message;
}

void assert_streq(const char *s1, const char *s2) {
  if (strcmp(s1, s2) != 0) {
    fprintf(stderr, "Expected \"%s\" to equal \"%s\".\n", s1, s2);
    abort();
  }
}

void test_noosphere() {
  printf("test_noosphere...\n");
  const char *hello_message = "Hello, Subconscious";

  ns_noosphere_t *noosphere = ns_initialize("/tmp/foo", "/tmp/bar", NULL, NULL);

  ns_tracing_initialize(NS_NOOSPHERE_LOG_ACADEMIC);

  ns_key_create(noosphere, "bob", NULL);
  ns_sphere_receipt_t *sphere_receipt =
      ns_sphere_create(noosphere, "bob", NULL);

  char *sphere_identity = ns_sphere_receipt_identity(sphere_receipt, NULL);
  char *sphere_mnemonic = ns_sphere_receipt_mnemonic(sphere_receipt, NULL);
  // printf("Sphere identity: %s\n", sphere_identity);
  // printf("Recovery code: %s\n", sphere_mnemonic);

  ns_sphere_t *sphere = ns_sphere_open(noosphere, sphere_identity, NULL);
  slice_ref_uint8_t data = {(uint8_t *)hello_message, strlen(hello_message)};

  ns_sphere_content_write(noosphere, sphere, "hello", "text/subtext", data,
                          NULL, NULL);
  ns_sphere_save(noosphere, sphere, NULL, NULL);

  ns_sphere_file_t *file =
      ns_sphere_content_read_blocking(noosphere, sphere, "/hello", NULL);

  slice_boxed_char_ptr_t headers =
      ns_sphere_file_header_values_read(file, "Content-Type");

  const char *expected_headers[1] = {"text/subtext"};

  assert(headers.len ==
         (sizeof(expected_headers) / sizeof(expected_headers[0])));
  for (int i = 0; i < headers.len; i++) {
    assert(strcmp(headers.ptr[i], expected_headers[i]) == 0);
  }

  slice_boxed_uint8_t contents =
      ns_sphere_file_contents_read_blocking(noosphere, file, NULL);
  char *contents_str = str_from_buffer(contents);
  assert_streq(contents_str, hello_message);

  free(contents_str);
  ns_string_array_free(headers);
  ns_bytes_free(contents);
  ns_sphere_file_free(file);
  ns_sphere_free(sphere);
  ns_string_free(sphere_identity);
  ns_string_free(sphere_mnemonic);
  ns_sphere_receipt_free(sphere_receipt);
  ns_free(noosphere);
}

void test_errors() {
  printf("test_errors...\n");
  ns_noosphere_t *noosphere = ns_initialize("/tmp/foo", "/tmp/bar", NULL, NULL);

  const char *sphere_identity = "doesnotexist";
  ns_error_t *error;
  ns_sphere_t *sphere = ns_sphere_open(noosphere, sphere_identity, &error);

  assert(sphere == NULL);
  assert(error != NULL);

  char *error_message = ns_error_message_get(error);
  assert(error_message != NULL);
  assert(ns_error_code_get(error) == NS_ERROR_CODE_OTHER);

  ns_error_free(error);
  ns_string_free(error_message);
  ns_free(noosphere);
}

int main() {
  setbuf(stdout, NULL);
  test_noosphere();
  test_errors();

  printf("Success.\n");
  return 0;
}
