// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile

import dev.typvia.mobile.ffi.TypviaStore
import dev.typvia.mobile.ui.Translator
import uniffi.typvia_mobile_ffi.AiProviderDraft
import uniffi.typvia_mobile_ffi.AiProviderRow
import java.net.URI

/**
 * Which machine an AI action's text would be handed to.
 *
 * The choice is the whole point of the chapter: it decides how far a selected
 * piece of text travels. Whichever is chosen, a snippet from the vault is
 * never sent to any model — that refusal lives behind the bridge and is not a
 * setting.
 */
enum class AiEngine {
    OnThisDevice,
    OwnKey,
    Off,
    ;

    companion object {
        /**
         * Read from what is actually configured rather than from a preference
         * of its own: a settings screen that keeps its own copy of the truth
         * eventually disagrees with it.
         */
        fun inEffect(providers: List<AiProviderRow>?): AiEngine {
            val provider = providers?.firstOrNull() ?: return Off
            return if (isLocal(provider.baseUrl)) OnThisDevice else OwnKey
        }

        /** An address that never leaves the machine. */
        fun isLocal(baseUrl: String): Boolean {
            val host = runCatching { URI(baseUrl).host }.getOrNull()?.lowercase() ?: return false
            return host == "localhost" || host == "127.0.0.1" || host == "::1"
        }
    }
}

/** What the AI chapter says, as values. */
object AiChapterCopy {
    fun title(engine: AiEngine, tr: Translator): String = when (engine) {
        AiEngine.OnThisDevice -> tr("On this device", "就在这台设备上")
        AiEngine.OwnKey -> tr("With my own API key", "用我自己的 API 密钥")
        AiEngine.Off -> tr("Turn AI actions off", "关掉 AI 动作")
    }

    /**
     * The line under each choice.
     *
     * The delivery gives the third one as "the AI chapter disappears; the
     * other seven kinds carry on" — which is not true of this build: the
     * library lists all eight kinds whichever engine is chosen. What is
     * printed here is what actually happens.
     */
    fun detail(engine: AiEngine, tr: Translator): String = when (engine) {
        AiEngine.OnThisDevice -> tr(
            "The text does not leave. Speed depends on the machine; long text is slower.",
            "文字不出门。速度取决于机器,长文会慢一些。",
        )
        AiEngine.OwnKey -> tr(
            "Through your own account. The key is kept by this device's key store and never syncs.",
            "走你自己的账号。密钥存在这台设备的钥匙存放处,不随同步上云。",
        )
        AiEngine.Off -> tr(
            "No text from this device goes to any model. The key is removed with it.",
            "这台设备上的文字不会再交给任何模型。密钥也一并删掉。",
        )
    }

    /**
     * Why a change did not take. Kinds, never the engine's own words.
     *
     * Every sentence here holds the same line: **a change that did not take
     * leaves the engine exactly as it was**. The page must never read as "off"
     * over something still configured, so none of these says or implies that
     * anything was removed.
     */
    fun refusal(kind: AiRefusal, tr: Translator): String = when (kind) {
        AiRefusal.NotUsable -> tr(
            "That address or model is not one the engine can use.",
            "这个地址或模型引擎用不了。",
        )
        AiRefusal.NoResult -> tr(
            "That did not take. The engine is as it was.",
            "没改成。引擎还是原来的样子。",
        )
        AiRefusal.Unreachable -> tr(
            "The engine could not be reached. The engine is as it was.",
            "没能连上引擎。引擎还是原来的样子。",
        )
        AiRefusal.NotPermitted -> tr(
            "This needs something it has not been given. The engine is as it was.",
            "这件事需要一项它还没有的东西。引擎还是原来的样子。",
        )
        AiRefusal.Missing -> tr(
            "That engine is no longer here.",
            "这个引擎已经不在了。",
        )
        AiRefusal.Storage -> tr(
            "That did not take. The engine is as it was.",
            "没改成。引擎还是原来的样子。",
        )
    }
}

/**
 * The calls the AI chapter makes.
 *
 * The API key passes through here and stops: it goes straight to the platform's
 * key store behind the bridge, is never written to the database, never logged,
 * and never carried by a sync round. Nothing in this file prints it, and
 * nothing in this file keeps it.
 */
object AiKeeperSettings {
    /** Local engines have a default endpoint of their own; naming it here would let it drift. */
    private const val LOCAL_KIND = "ollama"
    private const val ACCOUNT_KIND = "openai_compatible"

    suspend fun providers(store: TypviaStore): List<AiProviderRow>? =
        runCatching { store.perform { it.aiProviderList() } }.getOrNull()

    /**
     * Puts the chosen engine into effect.
     *
     * The choice is not a preference this page keeps — it is the set of
     * providers, which is what the rest of the product reads. Choosing "off"
     * removes them; choosing a machine writes one.
     */
    suspend fun choose(
        store: TypviaStore,
        engine: AiEngine,
        model: String,
        baseUrl: String,
        apiKey: String?,
    ): Result<Unit> = runCatching {
        val existing = providers(store).orEmpty()
        when (engine) {
            AiEngine.Off -> store.perform { core ->
                // The bridge takes the key out before the row, so a failure
                // here leaves an engine the reader can still see and retry —
                // never a credential with nothing pointing at it.
                for (provider in existing) core.aiProviderDelete(provider.id)
            }
            AiEngine.OnThisDevice, AiEngine.OwnKey -> store.perform { core ->
                val draft = AiProviderDraft(
                    id = existing.firstOrNull()?.id,
                    name = if (engine == AiEngine.OnThisDevice) "Local" else "My account",
                    // A local daemon and a hosted account are different kinds,
                    // not one kind with two addresses.
                    kind = if (engine == AiEngine.OnThisDevice) LOCAL_KIND else ACCOUNT_KIND,
                    // Empty means "the kind's own default endpoint".
                    baseUrl = baseUrl.trim(),
                    // A provider without a model is not a provider, and the
                    // engine says so. This does not invent one on the reader's
                    // behalf; it keeps whatever was already there.
                    model = model.trim().ifEmpty { existing.firstOrNull()?.model ?: "" },
                    timeoutMs = 0,
                )
                val saved = core.aiProviderSave(draft)
                if (!apiKey.isNullOrEmpty()) {
                    core.aiApiKeySet(saved.id, apiKey)
                } else if (engine == AiEngine.OnThisDevice) {
                    // A local engine needs no key, and leaving a stale one
                    // behind would keep a secret that nothing uses.
                    core.aiApiKeyClear(saved.id)
                }
            }
        }
    }
}
