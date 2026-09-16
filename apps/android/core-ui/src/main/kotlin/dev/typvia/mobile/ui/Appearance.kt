// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile.ui

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.compositionLocalOf

/**
 * The two things a reader can set about how the product looks.
 *
 * Both are preferences about the interface and nothing else — no snippet ever
 * passes through here. The stored words are the same three the desktop and the
 * other phone store (`system` / `en` / `zh`, `system` / `light` / `dark`), so a
 * reader who uses two of this product's ends does not have to learn that they
 * disagree about what "system" is called.
 */
object UiPreference {
    /** Which language the interface is rendered in. */
    enum class Language(val stored: String) {
        /** Whatever the phone is set to. */
        System("system"),
        En("en"),
        Zh("zh"),
        ;

        /** Resolves to the one that actually gets rendered. */
        fun resolved(tags: List<String>): UiLanguage = when (this) {
            System -> UiLanguage.resolve(tags)
            En -> UiLanguage.En
            Zh -> UiLanguage.Zh
        }

        companion object {
            /**
             * Reads back what was stored. Never set, and a word this build does
             * not know, are the same answer: follow the phone. A preference
             * written by a later version must not stop this one from starting.
             */
            fun of(stored: String?): Language =
                entries.firstOrNull { it.stored == stored } ?: System
        }
    }

    /** Light, dark, or the phone's own. */
    enum class Appearance(val stored: String) {
        System("system"),
        Light("light"),
        Dark("dark"),
        ;

        fun isDark(systemIsDark: Boolean): Boolean = when (this) {
            System -> systemIsDark
            Light -> false
            Dark -> true
        }

        companion object {
            /** As [Language.of]: unreadable and unset both mean the phone's own. */
            fun of(stored: String?): Appearance =
                entries.firstOrNull { it.stored == stored } ?: System
        }
    }
}

/**
 * The appearance the subtree is drawn in.
 *
 * It defaults to the phone's own, which is what every surface did before there
 * was anything to choose: a caller that does not provide it is not broken, it
 * simply has no reader to ask.
 */
val LocalAppearance = compositionLocalOf { UiPreference.Appearance.System }

/**
 * Whether the paper is dark here.
 *
 * The one place that question is answered. It used to be asked of the system
 * at each call site — five of them in [Paper] alone — which is why there was
 * nowhere for a reader's choice to go: **a setting can only override a decision
 * that is made in one place**.
 */
val isDarkPaper: Boolean
    @Composable @ReadOnlyComposable get() =
        LocalAppearance.current.isDark(isSystemInDarkTheme())
