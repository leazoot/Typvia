// Hand-written C ABI surface of the spike library (path B).
#pragma once
#include <stdint.h>

int64_t typvia_snapshot_count(const char *json);
char *typvia_first_title(const char *json);
void typvia_string_free(char *s);
