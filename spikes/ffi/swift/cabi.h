// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// Hand-written C ABI surface of the spike library (path B).
#pragma once
#include <stdint.h>

int64_t typvia_snapshot_count(const char *json);
char *typvia_first_title(const char *json);
void typvia_string_free(char *s);
