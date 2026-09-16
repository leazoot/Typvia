// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile

import dev.typvia.mobile.ui.Translator
import dev.typvia.mobile.ui.UiPreference
import dev.typvia.mobile.ui.pieces

/** What the contents page can say about each chapter without opening it. */
data class SettingsSummary(
    val total: UInt,
    val isSyncConfigured: Boolean,
    val deviceCount: Int,
    val vaultPhase: VaultPhase,
    /**
     * Whether this product's keyboard is switched on in the system. Null when
     * it could not be read — which is not the same as "off", and must not be
     * drawn as if it were.
     */
    val isKeyboardOn: Boolean? = null,
    /** How many AI engines are configured. Null when the list could not be read. */
    val aiEngines: Int? = null,
)

/**
 * The chapters of the settings room.
 *
 * A chapter that has no page yet is still printed — the contents page is the
 * product's map of itself, and leaving a chapter off it would say the product
 * does not do that thing. What it must not do is pretend to be openable.
 */
enum class SettingsChapter {
    Sync,
    Keyboard,
    Ai,
    Vault,
    Appearance,
    Data,
    ;

    fun title(tr: Translator): String = when (this) {
        Sync -> tr("Sync", "同步")
        Keyboard -> tr("Keyboard", "键盘")
        Ai -> tr("AI engine", "AI 引擎")
        Vault -> tr("Vault", "保险库")
        Appearance -> tr("Look & language", "外观与语言")
        Data -> tr("Your data", "你的数据")
    }

    /**
     * The one line under each title.
     *
     * It names what the chapter's page holds, and nothing else: a contents
     * page is where a reader decides whether it is worth opening, so a verb
     * printed here that the page does not have sends them looking for it.
     */
    fun blurb(tr: Translator): String = when (this) {
        Sync -> tr("End to end; the key never leaves your devices.", "端到端加密,钥匙不离开你的设备。")
        Keyboard -> tr("Trigger marks · order · what it may read", "触发符 · 排序 · 它能读到什么")
        Ai -> tr("Which machine the rewriting runs on", "改写与生成在哪台机器上跑")
        Vault -> tr("How it opens · when it shuts", "解锁方式 · 自动回锁")
        Appearance -> tr("Theme · text size · interface language", "主题 · 字号 · 界面语言")
        Data -> tr("Export · import · the bin", "导出 · 导入 · 回收站")
    }

    /**
     * The right column: what is true now, in the fewest words that are true.
     *
     * The reader's language is passed in rather than read from [summary]: the
     * summary is re-assembled when the library changes, and a preference kept
     * in it would go on printing the old answer until something unrelated
     * happened to be saved.
     */
    fun value(
        summary: SettingsSummary,
        language: UiPreference.Language,
        tr: Translator,
    ): String = when (this) {
        Sync -> when {
            !summary.isSyncConfigured -> tr("not set up", "未设置")
            summary.deviceCount <= 1 -> tr("this device only", "只有这台")
            else -> tr("${summary.deviceCount} devices", "${summary.deviceCount} 台设备")
        }
        // Null is not "off": a value that could not be read is left blank,
        // because printing "off" would be this page answering a question it
        // never got an answer to.
        Keyboard -> when (summary.isKeyboardOn) {
            true -> tr("on", "已启用")
            false -> tr("not on yet", "未启用")
            null -> ""
        }
        Ai -> when (summary.aiEngines) {
            null -> ""
            0 -> tr("not set up", "未设置")
            else -> tr("${summary.aiEngines} set up", "已配置 ${summary.aiEngines} 个")
        }
        Vault -> when (summary.vaultPhase) {
            VaultPhase.Absent -> tr("none yet", "还没有")
            VaultPhase.Shut -> tr("shut", "已锁")
            VaultPhase.Open -> tr("open", "已开")
        }
        Appearance -> AppearanceChapterCopy.contentsValue(language, tr)
        Data -> tr.pieces(summary.total.toInt())
    }
}
