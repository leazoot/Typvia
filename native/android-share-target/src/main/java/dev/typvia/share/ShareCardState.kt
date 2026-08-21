// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// Pure derivation of the card's honest state trio from the shared payload
// (mirror of the iOS card's showLoading/present/showUnavailable outcomes).
// Kept free of Android types so the host-JVM unit tests can pin every
// branch — including the >128 KB refusal, which no real sender on the
// emulator can produce (the shell caps a single argument at 128 KB and
// Chrome truncates shared selections at 100k characters).

package dev.typvia.share

/** What the card shows for a given payload: the meta line under the
 * preview and whether Save is offered. */
data class ShareCardState(val metaLine: String, val saveEnabled: Boolean)

/** [text] is the raw EXTRA_TEXT payload; null means the intent carried no
 * text at all (wrong action/type or a missing extra). */
fun shareCardState(text: String?): ShareCardState {
    if (text == null) {
        return ShareCardState("Nothing to save — this share did not include text", false)
    }
    if (text.isBlank()) {
        return ShareCardState("Nothing to save — the share holds no text", false)
    }
    val count = text.codePointCount(0, text.length)
    val counted = "$count character" + if (count == 1) "" else "s"
    if (text.toByteArray(Charsets.UTF_8).size > ShareInbox.TEXT_MAX_BYTES) {
        return ShareCardState("$counted · over the 128 KB limit, too long to save", false)
    }
    return ShareCardState(counted, true)
}
