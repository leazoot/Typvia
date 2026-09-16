// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.ui.graphics.Color

/**
 * The three papers and the three inks.
 *
 * Three levels of grey and no fourth: hierarchy below that is carried by size,
 * weight, letter-spacing and the mono face, never by a paler grey.
 */
object Paper {
    val base: Color @Composable @ReadOnlyComposable get() = pick(Tokens.Swatch.paperLight, Tokens.Swatch.paperDark)
    val carrier: Color @Composable @ReadOnlyComposable get() = pick(Tokens.Swatch.carrierLight, Tokens.Swatch.carrierDark)
    val ink: Color @Composable @ReadOnlyComposable get() = pick(Tokens.Swatch.inkLight, Tokens.Swatch.inkDark)
    val ink2: Color @Composable @ReadOnlyComposable get() = pick(Tokens.Swatch.ink2Light, Tokens.Swatch.ink2Dark)
    val ink3: Color @Composable @ReadOnlyComposable get() = pick(Tokens.Swatch.ink3Light, Tokens.Swatch.ink3Dark)

    /** The rule colour: ink, at the opacity the delivery gives rules. */
    val rule: Color
        @Composable @ReadOnlyComposable get() = ink.copy(
            alpha = if (isDarkPaper) {
                Tokens.Line.HAIRLINE_OPACITY_DARK
            } else {
                Tokens.Line.HAIRLINE_OPACITY_LIGHT
            },
        )

    /** The one warm accent, used where something wants attention without alarm. */
    val attention: Color @Composable @ReadOnlyComposable get() = pick(Tokens.Swatch.aiLight, Tokens.Swatch.aiDark)

    @Composable
    @ReadOnlyComposable
    private fun pick(light: Color, dark: Color): Color = if (isDarkPaper) dark else light
}

/** The vault's own room: ink in either theme, because the material is the point. */
object VaultRoom {
    val base = Tokens.Swatch.paperDark
    val carrier = Tokens.Swatch.carrierDark
    val ink = Tokens.Swatch.inkDark
    val ink2 = Tokens.Swatch.ink2Dark
    val ink3 = Tokens.Swatch.ink3Dark
    val accent = Tokens.Swatch.vaultDark
}

/**
 * Which room a screen is in. The room decides one thing — the accent — so a
 * screen states its identity once instead of at every caret.
 */
enum class Room {
    Home,
    Library,
    Vault,
    Settings,
    Ai,
    ;

    val accent: Color
        @Composable @ReadOnlyComposable get() {
            val dark = isDarkPaper
            return when (this) {
                Home -> if (dark) Tokens.Swatch.homeDark else Tokens.Swatch.homeLight
                Library -> if (dark) Tokens.Swatch.libraryDark else Tokens.Swatch.libraryLight
                Vault -> if (dark) Tokens.Swatch.vaultDark else Tokens.Swatch.vaultLight
                Settings -> if (dark) Tokens.Swatch.settingsDark else Tokens.Swatch.settingsLight
                Ai -> if (dark) Tokens.Swatch.aiDark else Tokens.Swatch.aiLight
            }
        }
}

/** The room the subtree belongs to. */
val LocalRoom = compositionLocalOf { Room.Home }

/**
 * What stands in for words nobody is allowed to see.
 *
 * One run of dots, in one place. It was written out three times — in the vault
 * room, on the keyboard's locked sheet, and on the page that reads a secret —
 * and three copies of a thing that means "there is something here" is three
 * chances for one of them to mean something slightly different.
 */
object BodyMask {
    const val SHORT = "·······"
}
