// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import XCTest

@testable import TypviaKit

/// The settings room's rules: which chapters open, what "which engine" is read
/// from, and how a device row reports what it is doing.
final class SettingsTests: XCTestCase {
    // MARK: - The contents page

    func testTheChaptersAreNumberedInTheDeliverysOrder() {
        XCTAssertEqual(
            SettingsChapter.allCases.map(\.number),
            ["01", "02", "03", "04", "05", "06"]
        )
        XCTAssertEqual(SettingsChapter.allCases.first, .sync)
        XCTAssertEqual(SettingsChapter.allCases.last, .data)
    }

    /// Every row on the contents page opens something now. The page used to
    /// carry rows that were print only, which is why the model refused to open
    /// them; there is nothing left to refuse.
    @MainActor
    func testEveryChapterOpens() throws {
        let model = SettingsModel(store: try TypviaStore(dataDirectory: temporary()))
        for chapter in SettingsChapter.allCases {
            model.openChapter(chapter)
            XCTAssertEqual(model.open, chapter)
        }
        model.closeChapter()
        XCTAssertNil(model.open)
    }

    /// The keyboard chapter's value column never prints "off" over a question
    /// this device did not answer: the list it reads is not always given to
    /// us, so "not there" and "could not see" arrive the same way.
    func testTheKeyboardValueNeverClaimsItIsOff() {
        for tr in [Translator(language: .en), Translator(language: .zh)] {
            let unconfirmed = KeyboardChapterCopy.contentsValue(.notAdded, tr)
            XCTAssertNotEqual(unconfirmed, KeyboardChapterCopy.contentsValue(.ready, tr))
            XCTAssertFalse(unconfirmed.lowercased().contains("off"))
            XCTAssertFalse(unconfirmed.contains("已关闭"))
        }
        XCTAssertNotEqual(
            KeyboardChapterCopy.headline(.notAdded, Translator(language: .en)),
            KeyboardChapterCopy.headline(.notAdded, Translator(language: .zh))
        )
    }

    /// The chapter names what this product does about full access, which is
    /// nothing: the switch exists in the system and Typvia does not ask for
    /// it. A page that stayed quiet about it would leave the reader to guess.
    func testTheKeyboardChapterSaysWhatFullAccessCostsHere() {
        let en = KeyboardChapterCopy.headline(.ready, Translator(language: .en))
        XCTAssertFalse(en.contains("full access"), "the headline is about being added, not access")
    }

    private func temporary() -> URL {
        URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent(UUID().uuidString)
    }

    /// The vault chapter's line names three things, and the page states all
    /// three. It used to name two, and the third — what a screenshot does —
    /// was in the delivery and in this build's comments but nowhere a reader
    /// could find it.
    func testTheVaultChapterNamesEverythingItsPageStates() {
        XCTAssertEqual(
            SettingsCopy.summary(.vault, tr: Translator(language: .en)),
            "How it opens · when it shuts · screenshots"
        )
        XCTAssertEqual(
            SettingsCopy.summary(.vault, tr: Translator(language: .zh)),
            "解锁方式 · 自动回锁 · 截屏保护"
        )
    }

    /// This line used to offer a third verb — "delete everything" — that the
    /// data page does not have. A reader scans the contents page to decide
    /// what to open, so a verb printed here is a reader sent looking for it.
    func testTheDataChapterOffersOnlyWhatItsPageHolds() {
        XCTAssertEqual(
            SettingsCopy.summary(.data, tr: Translator(language: .en)),
            "Export · import · the recycle bin"
        )
        XCTAssertEqual(
            SettingsCopy.summary(.data, tr: Translator(language: .zh)),
            "导出 · 导入 · 回收站"
        )
    }

    // MARK: - What the AI paths say when something is refused

    /// The shared sentence for a broken rule is "something here is already
    /// taken". On both AI paths that kind means a missing key, an engine
    /// saying no, or the gate refusing to send — so borrowing it says the one
    /// thing that is certainly not happening.
    func testNoAiSentenceBorrowsTheOneWrittenForATakenTrigger() {
        for language in UiLanguage.allCases {
            let tr = Translator(language: language)
            let shared = Refusal.clash.sentence(tr)
            for refusal in [Refusal.invalid, .clash, .notPermitted, .ruleBlocked, .offline, .missing, .storage] {
                XCTAssertNotEqual(SettingsCopy.engineRefusal(refusal, tr: tr), shared)
                XCTAssertNotEqual(AiStripCopy.refusal(refusal, tr), shared)
            }
        }
    }

    /// The refusal this product would most like to name — the gate holding
    /// text back — arrives folded together with "no key" and "the provider
    /// said no". So the sentence for that kind must not claim anything either
    /// way about what left the device.
    func testTheFoldedRefusalClaimsNothingAboutWhatLeft() {
        for language in UiLanguage.allCases {
            let tr = Translator(language: language)
            let said = AiStripCopy.refusal(.clash, tr).lowercased()
            XCTAssertFalse(said.contains("sent"))
            XCTAssertFalse(said.contains("left"))
            XCTAssertFalse(said.contains("送出"))
            XCTAssertFalse(said.contains("出门"))
        }
    }

    /// A change that did not take leaves the engine as it was, and the chapter
    /// must never read as "off" over one that is still configured.
    func testNoEngineSentenceReadsAsOffOverAnEngineStillThere() {
        for language in UiLanguage.allCases {
            let tr = Translator(language: language)
            for refusal in [Refusal.invalid, .clash, .notPermitted, .ruleBlocked, .offline, .storage] {
                let said = SettingsCopy.engineRefusal(refusal, tr: tr)
                XCTAssertTrue(
                    said.contains("as it was") || said.contains("还是原来的样子")
                        || said.contains("engine can use") || said.contains("引擎用不了"),
                    said
                )
            }
        }
    }

    /// The delivery says turning AI off makes the AI chapter disappear and
    /// leaves seven kinds. Neither is true of this build: the library lists
    /// all eight kinds whichever engine is chosen.
    func testTurningItOffDescribesWhatActuallyHappens() {
        for language in UiLanguage.allCases {
            let said = SettingsCopy.engineDetail(.off, tr: Translator(language: language))
            XCTAssertFalse(said.lowercased().contains("chapter"))
            XCTAssertFalse(said.contains("章节"))
            XCTAssertFalse(said.contains("seven"))
            XCTAssertFalse(said.contains("7"))
        }
    }

    // MARK: - Which engine is in effect

    func testNoProviderMeansAiIsOff() {
        XCTAssertEqual(AiEngine.inEffect(providers: []), .off)
    }

    func testAProviderOnThisMachineIsLocal() {
        for host in ["http://127.0.0.1:11434/v1", "http://localhost:11434", "http://[::1]:8080"] {
            XCTAssertEqual(
                AiEngine.inEffect(providers: [provider(baseUrl: host)]),
                .onThisDevice,
                host
            )
        }
    }

    func testAProviderSomewhereElseIsTheReadersOwnAccount() {
        XCTAssertEqual(
            AiEngine.inEffect(providers: [provider(baseUrl: "https://api.example.com/v1")]),
            .ownKey
        )
    }

    func testAMalformedBaseUrlIsNotTreatedAsLocal() {
        // Failing open here would tell a reader their text stays on the device
        // when it does not.
        XCTAssertEqual(AiEngine.inEffect(providers: [provider(baseUrl: "not a url")]), .ownKey)
        XCTAssertEqual(AiEngine.inEffect(providers: [provider(baseUrl: "")]), .ownKey)
    }

    func testAHostThatMerelyLooksLocalIsNotLocal() {
        XCTAssertEqual(
            AiEngine.inEffect(providers: [provider(baseUrl: "https://localhost.example.com/v1")]),
            .ownKey
        )
    }

    // MARK: - Device rows

    func testThisDeviceSaysWhatItIsDoing() {
        let quiet = DeviceRow(device: device(isThis: true), pending: 0)
        XCTAssertEqual(quiet.state, .inUse)
        let busy = DeviceRow(device: device(isThis: true), pending: 7)
        XCTAssertEqual(busy.state, .waiting(7))
    }

    func testAnotherDeviceReportsWhenItWasLastHeardFrom() {
        let other = DeviceRow(device: device(isThis: false, createdAt: 42), pending: 7)
        XCTAssertEqual(other.state, .lastSeen(42))
    }

    func testARevokedDeviceCarriesNoTimestamp() {
        let revoked = DeviceRow(device: device(isThis: false, revokedAt: 99), pending: 0)
        XCTAssertEqual(revoked.state, .lastSeen(nil))
    }

    // MARK: - Fixtures

    private func provider(baseUrl: String) -> AiProviderRow {
        AiProviderRow(
            id: "p1", name: "Provider", kind: "openai_compat", baseUrl: baseUrl,
            model: "a-model", timeoutMs: 30_000, hasApiKey: true
        )
    }

    private func device(
        isThis: Bool, createdAt: Int64 = 0, revokedAt: Int64? = nil
    ) -> SyncDevice {
        SyncDevice(
            deviceId: isThis ? "this" : "other",
            name: isThis ? "iPhone" : "MacBook",
            platform: "ios",
            createdAt: createdAt,
            revokedAt: revokedAt,
            verified: true,
            isThisDevice: isThis,
            isRoot: false
        )
    }
}

/// The recycle bin. Its rules are about a deadline and about what a
/// confirmation has to say.
final class TrashTests: XCTestCase {
    private let day: Int64 = 86_400_000

    func testWhatIsReportedIsTheDeadlineNotHowLongAgoItWent() {
        let now: Int64 = 1_000 * day
        XCTAssertEqual(TrashWindow.daysLeft(deletedAt: now, now: now), 30)
        XCTAssertEqual(TrashWindow.daysLeft(deletedAt: now - 10 * day, now: now), 20)
        XCTAssertEqual(TrashWindow.daysLeft(deletedAt: now - 29 * day, now: now), 1)
    }

    func testAnOverdueRowReadsAsZeroRatherThanAsNegativeDays() {
        let now: Int64 = 1_000 * day
        XCTAssertEqual(TrashWindow.daysLeft(deletedAt: now - 40 * day, now: now), 0)
        XCTAssertEqual(TrashWindow.daysLeft(deletedAt: now - 30 * day, now: now), 0)
    }

    func testARowWithNoTimestampSaysSoRatherThanGuessing() {
        XCTAssertNil(TrashWindow.daysLeft(deletedAt: nil, now: 1_000 * day))
    }

    func testTheWindowMatchesTheOneTheCoreEnforces() {
        // Mirrored, not read — the bridge does not export it. If this ever
        // stops matching `TRASH_RETENTION_MS`, the screen misstates a deadline.
        XCTAssertEqual(TrashWindow.days, 30)
    }
}

/// The two UI preferences. They live in the App Group so the keyboard and the
/// widgets render in the same language as the app — a keyboard that ignores
/// the app's language setting is the mixed-language screen the design forbids,
/// arriving through the back door.
@MainActor
final class UiPreferencesTests: XCTestCase {
    private var defaults: UserDefaults!
    private var suite: String!

    override func setUpWithError() throws {
        suite = "test.\(UUID().uuidString)"
        defaults = UserDefaults(suiteName: suite)
    }

    override func tearDownWithError() throws {
        defaults.removePersistentDomain(forName: suite)
    }

    func testAnUnsetPreferenceFollowsTheSystem() {
        let prefs = UiPreferences(defaults: defaults)
        XCTAssertEqual(prefs.language, .system)
        XCTAssertEqual(prefs.appearance, .system)
    }

    func testAStoreThatCannotBeOpenedStillStarts() {
        // A failure to read a preference must not stop the product.
        let prefs = UiPreferences(defaults: nil)
        XCTAssertEqual(prefs.language, .system)
        XCTAssertEqual(prefs.appearance, .system)
    }

    func testAChoiceSurvivesTheNextLaunch() {
        let prefs = UiPreferences(defaults: defaults)
        prefs.language = .zh
        prefs.appearance = .dark
        let reopened = UiPreferences(defaults: defaults)
        XCTAssertEqual(reopened.language, .zh)
        XCTAssertEqual(reopened.appearance, .dark)
    }

    func testTheKeysAreTheOnesTheDesktopUses() {
        // So that the two products read as one when someone goes looking.
        XCTAssertEqual(UiPreferences.Keys.language, "tv.ui.locale")
        XCTAssertEqual(UiPreferences.Keys.appearance, "tv.ui.theme")
    }

    func testAnExplicitLanguageWinsOverTheSystems() {
        XCTAssertEqual(UiPreference.Language.zh.resolved(preferred: ["en-US"]), .zh)
        XCTAssertEqual(UiPreference.Language.en.resolved(preferred: ["zh-Hans-CN"]), .en)
        XCTAssertEqual(UiPreference.Language.system.resolved(preferred: ["zh-Hans-CN"]), .zh)
    }

    func testAGarbledStoredValueFallsBackRatherThanCrashing() {
        defaults.set("klingon", forKey: UiPreferences.Keys.language)
        defaults.set("neon", forKey: UiPreferences.Keys.appearance)
        let prefs = UiPreferences(defaults: defaults)
        XCTAssertEqual(prefs.language, .system)
        XCTAssertEqual(prefs.appearance, .system)
    }

    /// A widget is drawn from a value, in a process with nothing to observe.
    /// It reads through the same parse as everything else, so an unknown word
    /// cannot fall back one way in the app and another way on a home screen.
    func testTheValueReadSeesWhatTheObservedOneSees() {
        let prefs = UiPreferences(defaults: defaults)
        prefs.language = .zh
        prefs.appearance = .dark
        XCTAssertEqual(
            UiPreferences.chosen(defaults: defaults),
            UiPreferences.Chosen(language: .zh, appearance: .dark)
        )
        defaults.set("neon", forKey: UiPreferences.Keys.appearance)
        XCTAssertEqual(UiPreferences.chosen(defaults: defaults).appearance, .system)
        XCTAssertEqual(UiPreferences.chosen(defaults: nil), UiPreferences.Chosen())
    }

    /// Following the phone has to stay a third state. Folding it into one of
    /// the two would make "System" mean light on every surface that reads the
    /// value rather than the window.
    func testFollowingThePhoneIsNotEitherOfTheTwoSchemes() {
        XCTAssertNil(UiPreference.Appearance.system.colorScheme)
        XCTAssertEqual(UiPreference.Appearance.dark.colorScheme, .dark)
        XCTAssertEqual(UiPreference.Appearance.light.colorScheme, .light)
    }
}

/// Choosing an AI engine, against a real database.
///
/// The choice is not a preference kept beside the truth — it *is* the set of
/// providers, which is what the gate and the log read.
@MainActor
final class AiEngineChoiceTests: XCTestCase {
    private var directory: URL!
    private var store: TypviaStore!

    override func setUpWithError() throws {
        directory = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent(UUID().uuidString)
        store = try TypviaStore(dataDirectory: directory)
    }

    override func tearDownWithError() throws {
        store = nil
        try? FileManager.default.removeItem(at: directory)
    }

    func testAFreshLibraryHasNoEngine() async {
        let model = SettingsModel(store: store)
        await model.load()
        XCTAssertEqual(model.engine, .off)
    }

    func testChoosingThisDeviceWritesAProviderThatNeverLeavesIt() async throws {
        let model = SettingsModel(store: store)
        await model.choose(.onThisDevice, model: "llama3.2")
        XCTAssertEqual(model.engine, .onThisDevice)

        let providers = try await store.perform { try $0.aiProviderList() }
        XCTAssertEqual(providers.count, 1)
        XCTAssertTrue(AiEngine.isLocal(providers[0].baseUrl), providers[0].baseUrl)
    }

    func testTurningItOffNeverShowsOffOverAnEngineThatIsStillThere() async throws {
        let model = SettingsModel(store: store)
        await model.choose(.onThisDevice, model: "llama3.2")
        await model.choose(.off)

        // Removing a provider takes its key out of the secure store first, and
        // this host has none — so the removal may fail. What must hold either
        // way is that the screen does not lie: off means gone, and a failure
        // says so instead of pretending.
        let providers = try await store.perform { try $0.aiProviderList() }
        if providers.isEmpty {
            XCTAssertEqual(model.engine, .off)
            XCTAssertNil(model.refusal)
        } else {
            XCTAssertNotEqual(model.engine, .off, "no optimistic 'off' over a live engine")
            XCTAssertNotNil(model.refusal, "and the reader is told why")
        }
    }

    func testSwitchingToAnAccountReplacesRatherThanAccumulates() async throws {
        let model = SettingsModel(store: store)
        await model.choose(.onThisDevice, model: "llama3.2")
        // A hosted account needs its endpoint: the engine says so, and the
        // chapter asks for it rather than inventing one.
        await model.choose(.ownKey, model: "gpt-4o-mini", baseUrl: "https://api.example.com/v1")
        XCTAssertNil(model.refusal, "refusal was: \(String(describing: model.refusal))")
        let providers = try await store.perform { try $0.aiProviderList() }
        XCTAssertEqual(providers.count, 1, "two engines configured at once is a state nobody chose")
        XCTAssertEqual(model.engine, .ownKey)
    }
}
