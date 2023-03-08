#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "noosphere.h"

#ifdef NDEBUG
#error Asserting asserts are asserted.
#endif

slice_ref_uint8_t str_to_buffer(const char *text)
{
  size_t message_len = strlen(text);
  char *message = (char *)malloc(message_len + 1);
  snprintf(message, message_len + 1, "%s", text);
  slice_ref_uint8_t data = {(uint8_t *)message, message_len};
  return data;
}

void assert_streq(const char *s1, const char *s2)
{
  if (strcmp(s1, s2) != 0)
  {
    fprintf(stderr, "Expected \"%s\" to equal \"%s\".\n", s1, s2);
    abort();
  }
}

void test_noosphere()
{
  setbuf(stdout, NULL);
  const char *hello_message = "Hello, Subconscious";

  ns_noosphere_context_t *noosphere =
      ns_initialize("/tmp/foo", "/tmp/bar", NULL, NULL);

  ns_key_create(noosphere, "bob", NULL);
  ns_sphere_receipt_t *sphere_receipt = ns_sphere_create(noosphere, "bob", NULL);

  char *sphere_identity = ns_sphere_receipt_identity(sphere_receipt, NULL);
  char *sphere_mnemonic = ns_sphere_receipt_mnemonic(sphere_receipt, NULL);
  // printf("Sphere identity: %s\n", sphere_identity);
  // printf("Recovery code: %s\n", sphere_mnemonic);

  ns_sphere_fs_t *sphere_fs = ns_sphere_fs_open(noosphere, sphere_identity, NULL);
  slice_ref_uint8_t data = str_to_buffer(hello_message);

  ns_sphere_fs_write(noosphere, sphere_fs, "hello", "text/subtext", data, NULL,
                     NULL);
  ns_sphere_fs_save(noosphere, sphere_fs, NULL, NULL);

  ns_sphere_file_t *file =
      ns_sphere_fs_read(noosphere, sphere_fs, "/hello", NULL);

  slice_boxed_char_ptr_t headers =
      ns_sphere_file_header_values_read(file, "Content-Type");

  const char *expected_headers[1] = {
      "text/subtext"};

  assert(headers.len == (sizeof(expected_headers) / sizeof(expected_headers[0])));
  for (int i = 0; i < headers.len; i++)
  {
    assert(strcmp(headers.ptr[i], expected_headers[i]) == 0);
  }

  slice_boxed_uint8_t contents =
      ns_sphere_file_contents_read(noosphere, file, NULL);
  assert_streq((char *)contents.ptr, hello_message);

  ns_string_array_free(headers);
  ns_bytes_free(contents);
  ns_sphere_file_free(file);
  ns_sphere_fs_free(sphere_fs);
  ns_string_free(sphere_identity);
  ns_string_free(sphere_mnemonic);
  ns_sphere_receipt_free(sphere_receipt);
  ns_free(noosphere);
}

int main()
{
  test_noosphere();

  printf("Success.\n");
  return 0;
}
