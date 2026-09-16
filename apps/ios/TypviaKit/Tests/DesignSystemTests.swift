// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import SwiftUI
import XCTest

@testable import TypviaKit

/// The design system's rules, checked where they can be checked.
///
/// A SwiftUI view's pixels are not what these assert — a screenshot test would
/// re-encode the design rather than verify it. What they hold is the part that
/// can go quietly wrong: that a token resolves to the value the delivery names,
/// that the dark theme is not the light theme with a filter, that reduced
/// motion leaves the caret visible, and that the one mark allowed to be solid
/// is the one that means "this will not be previewed".
final class DesignSystemTests: XCTestCase {
    private let light = UITraitCollection(userInterfaceStyle: .light)
    private let dark = UITraitCollection(userInterfaceStyle: .dark)

    // MARK: Paper

    func testThreeLayersResolveToTheDeliverysValues() {
        XCTAssertEqual(hex(Paper.base, light), 0xFB_FA_F7)
        XCTAssertEqual(hex(Paper.carrier, light), 0xF3_F0_E9)
        XCTAssertEqual(hex(Paper.ink, light), 0x1A_19_17)

        XCTAssertEqual(hex(Paper.base, dark), 0x14_13_0F)
        XCTAssertEqual(hex(Paper.carrier, dark), 0x1E_1C_16)
        XCTAssertEqual(hex(Paper.ink, dark), 0xED_E6_D6)
    }

    func testInkIsNeverPureBlackOrPureWhite() {
        XCTAssertNotEqual(hex(Paper.ink, light), 0x00_00_00)
        XCTAssertNotEqual(hex(Paper.ink, dark), 0xFF_FF_FF)
        XCTAssertNotEqual(hex(Paper.base, light), 0xFF_FF_FF)
        XCTAssertNotEqual(hex(Paper.base, dark), 0x00_00_00)
    }

    func testTheThreeGreysAreThreeDistinctGreys() {
        for traits in [light, dark] {
            let greys = Set([hex(Paper.ink, traits), hex(Paper.ink2, traits), hex(Paper.ink3, traits)])
            XCTAssertEqual(greys.count, 3)
        }
    }

    func testRuleIsInkThinnedNotAGreyOfItsOwn() {
        XCTAssertEqual(hex(Paper.rule, light), hex(Paper.ink, light))
        XCTAssertEqual(hex(Paper.rule, dark), hex(Paper.ink, dark))
        XCTAssertEqual(alpha(Paper.rule, light), 0.08, accuracy: 0.005)
        XCTAssertEqual(alpha(Paper.rule, dark), 0.10, accuracy: 0.005)
    }

    // MARK: Rooms

    func testEveryRoomHasItsOwnAccentInBothThemes() {
        for traits in [light, dark] {
            let accents = Set(Room.allCases.map { hex($0.accent, traits) })
            XCTAssertEqual(accents.count, Room.allCases.count)
        }
    }

    func testAccentsBrightenInTheDark() {
        for room in Room.allCases {
            XCTAssertGreaterThan(
                luminance(room.accent, dark),
                luminance(room.accent, light),
                "\(room.rawValue) must lift in the dark rather than reuse its light value"
            )
        }
    }

    func testOnlyTheVaultWaitsSlowly() {
        for room in Room.allCases where room != .vault {
            XCTAssertEqual(room.breatheDuration, 1.6, accuracy: 0.0001)
        }
        XCTAssertEqual(Room.vault.breatheDuration, 2.4, accuracy: 0.0001)
    }

    func testTheVaultRoomStaysDarkUnderALightTheme() {
        // The vault is a dark room whatever the app's theme is, so its colours
        // must not answer the trait collection at all.
        XCTAssertEqual(hex(VaultRoom.base, light), hex(VaultRoom.base, dark))
        XCTAssertEqual(hex(VaultRoom.base, light), 0x14_13_0F)
        XCTAssertEqual(hex(VaultRoom.ink, light), 0xED_E6_D6)
        XCTAssertEqual(hex(VaultRoom.accent, light), 0x6F_A9_8B)
    }

    // MARK: Type sorts

    func testSecretIsTheOnlySolidMark() {
        let reversed = TypeSort.allCases.filter(\.isReversed)
        XCTAssertEqual(reversed, [.secret])
    }

    func testEverySortIsTwoDistinctUppercaseLetters() {
        let codes = TypeSort.allCases.map(\.code)
        XCTAssertEqual(Set(codes).count, codes.count)
        for code in codes {
            XCTAssertEqual(code.count, 2)
            XCTAssertEqual(code, code.uppercased())
        }
        XCTAssertEqual(codes, ["TX", "CD", "CM", "PR", "TP", "SC", "AI", "LK"])
    }

    func testMarkSizesAreFourDistinctSquares() {
        let sizes: [TypeSortSize] = [.compact, .row, .key, .chapter]
        XCTAssertEqual(Set(sizes.map(\.side)).count, sizes.count)
        for size in sizes {
            // The letters live inside the square, never overflow it.
            XCTAssertLessThan(size.labelSize, size.side)
        }
    }

    // MARK: Motion

    func testDepartureIsSevenTenthsOfArrival() {
        for beat in [Beat.tap, .state, .transition, .unlock] {
            XCTAssertEqual(
                beat.duration(reduceMotion: false, isExit: true),
                beat.duration(reduceMotion: false) * 0.7,
                accuracy: 0.0001
            )
        }
    }

    func testBeatsCarryTheDeliverysDurations() {
        XCTAssertEqual(Beat.tap.duration, 0.080, accuracy: 0.0001)
        XCTAssertEqual(Beat.state.duration, 0.160, accuracy: 0.0001)
        XCTAssertEqual(Beat.transition.duration, 0.240, accuracy: 0.0001)
        XCTAssertEqual(Beat.unlock.duration, 0.320, accuracy: 0.0001)
    }

    func testReducedMotionCollapsesEveryBeatToOneCrossFade() {
        for beat in [Beat.tap, .state, .transition, .unlock] {
            for isExit in [false, true] {
                XCTAssertEqual(
                    beat.duration(reduceMotion: true, isExit: isExit),
                    0.120,
                    accuracy: 0.0001,
                    "\(beat) must not keep a duration of its own under reduced motion"
                )
            }
        }
    }

    func testTypingOutIsCappedSoALongLineIsNotAPerformance() {
        XCTAssertEqual(Beat.typeInDuration(characterCount: 4), 0.104, accuracy: 0.0001)
        XCTAssertEqual(Beat.typeInDuration(characterCount: 400), 0.240, accuracy: 0.0001)
    }

    // MARK: Caret

    func testReducedMotionLeavesTheCaretPresentAndStill() {
        for behaviour in [Caret.Behaviour.breathing, .blinking, .still] {
            XCTAssertEqual(
                Caret.resolvedBehaviour(behaviour, reduceMotion: true),
                .still,
                "the caret must not animate, and must not vanish, under reduced motion"
            )
        }
    }

    func testCaretBehaviourIsUntouchedWhenMotionIsWelcome() {
        XCTAssertEqual(Caret.resolvedBehaviour(.breathing, reduceMotion: false), .breathing)
        XCTAssertEqual(Caret.resolvedBehaviour(.blinking, reduceMotion: false), .blinking)
    }

    func testCaretWidthFollowsItsHeight() {
        XCTAssertEqual(Caret.width(forHeight: 14), 2)
        XCTAssertEqual(Caret.width(forHeight: 19), 2)
        XCTAssertEqual(Caret.width(forHeight: 26), 3)
        XCTAssertEqual(Caret.width(forHeight: 34), 4)
        XCTAssertEqual(Caret.width(forHeight: 52), 4)
        XCTAssertEqual(Caret.width(forHeight: 74), 5)
    }

    func testBreathingNeverReachesZero() {
        // A mascot that disappears mid-cycle reads as a rendering fault.
        XCTAssertGreaterThan(Tokens.Caret.breatheFloor, 0)
    }

    // MARK: Type ladder

    func testTheLadderCarriesTheDeliverysSizesAtTheDefaultStep() {
        XCTAssertEqual(TypviaType.display.size(at: .large), 44, accuracy: 0.5)
        XCTAssertEqual(TypviaType.title1.size(at: .large), 34, accuracy: 0.5)
        XCTAssertEqual(TypviaType.body.size(at: .large), 17, accuracy: 0.5)
        XCTAssertEqual(TypviaType.caption.size(at: .large), 13, accuracy: 0.5)
        XCTAssertEqual(TypviaType.monoLabel.size(at: .large), 11, accuracy: 0.5)
    }

    func testTypeGrowsWithEveryStandardStep() {
        let steps: [DynamicTypeSize] = [.xSmall, .small, .medium, .large, .xLarge, .xxLarge, .xxxLarge]
        let sizes = steps.map { TypviaType.body.size(at: $0) }
        XCTAssertEqual(sizes, sizes.sorted())
        XCTAssertGreaterThan(sizes.last!, sizes.first!)
    }

    func testAccessibilityStepsAreClampedRatherThanFollowedForever() {
        let ceiling = TypviaType.body.size(at: .accessibility3)
        XCTAssertGreaterThan(ceiling, TypviaType.body.size(at: .xxxLarge))
        XCTAssertEqual(TypviaType.body.size(at: .accessibility4), ceiling)
        XCTAssertEqual(TypviaType.body.size(at: .accessibility5), ceiling)
    }

    func testLayoutStacksOnlyOnceTypeReachesAccessibilitySizes() {
        XCTAssertEqual(Reflow(for: .xxxLarge), .regular)
        XCTAssertEqual(Reflow(for: .accessibility1), .stacked)
        XCTAssertEqual(Reflow(for: .xxxLarge).titleBonus, 0)
        XCTAssertEqual(Reflow(for: .accessibility1).titleBonus, 4)
    }

    func testTrackingIsStatedInEmAndResolvedInPoints() {
        // display is -0.02em at 44pt.
        XCTAssertEqual(TypviaType.display.tracking(at: .large), -0.88, accuracy: 0.02)
        XCTAssertEqual(TypviaType.body.tracking(at: .large), 0, accuracy: 0.0001)
    }

    func testLineSpacingReachesTheDeliverysLineHeight() {
        let font = TypviaType.body.uiFont(at: .large)
        let total = font.lineHeight + TypviaType.body.lineSpacing(at: .large)
        XCTAssertEqual(total, font.pointSize * 1.55, accuracy: 0.5)
    }

    // MARK: The bundled face

    func testTheMonoFaceIsBundledRatherThanFallenBackTo() {
        // The fallback is real and deliberate, but a product that silently
        // takes it looks subtly wrong and nobody finds out. If this fails, the
        // font resources did not reach the bundle.
        XCTAssertTrue(TypviaFontResource.isBundled)
        XCTAssertEqual(TypviaType.mono.uiFont(at: .large).familyName, "JetBrains Mono")
        XCTAssertEqual(TypviaType.sortMark.uiFont(at: .large).familyName, "JetBrains Mono")
    }

    func testProseUsesThePlatformFaceSoChineseIsCovered() {
        // Latin falls to SF Pro, Chinese to PingFang SC — the delivery names
        // PingFang as its own Chinese choice, and no bundled Latin face could
        // cover CJK anyway.
        XCTAssertNotEqual(TypviaType.body.uiFont(at: .large).familyName, "JetBrains Mono")
    }

    // MARK: Lines and grain

    func testHairlineIsOnePixelAtTwoTimesAndStopsShortOfTheEdge() {
        XCTAssertEqual(Tokens.Line.hairlineWidth, 0.5)
        XCTAssertEqual(Tokens.Line.hairlineInset, 24)
        XCTAssertEqual(Tokens.Line.maxRulesPerScreen, 3)
    }

    func testSearchUnderlineHasExactlyTwoStates() {
        XCTAssertEqual(Tokens.Line.searchRestWidthFraction, 0.62)
        XCTAssertEqual(Tokens.Line.searchRestOpacity, 0.20, accuracy: 0.0001)
        XCTAssertEqual(Tokens.Line.searchFocusWidthFraction, 1.0)
        XCTAssertEqual(Tokens.Line.searchFocusOpacity, 0.45, accuracy: 0.0001)
        XCTAssertEqual(Tokens.Line.searchGrow, 0.160, accuracy: 0.0001)
    }

    func testGrainIsTiledAndHalvedInTheDark() {
        XCTAssertEqual(PaperGrain.tile.size.width, 120)
        XCTAssertEqual(PaperGrain.tile.size.height, 120)
        XCTAssertEqual(Tokens.Texture.opacityDark, Tokens.Texture.opacityLight / 2, accuracy: 0.0001)
        XCTAssertLessThan(Tokens.Texture.opacityLight, 0.03001)
    }

    // MARK: Counted nouns

    /// The plural rule lives in one place because written out at each call site
    /// it came out as "1 pieces", "1 matches", "1 devices".
    func testACountOfOneTakesTheSingularNounAndNothingElseDoes() {
        let en = Translator(language: .en)

        XCTAssertEqual(en.counted(1, "device", "devices", "1 台设备"), "1 device")
        XCTAssertEqual(en.counted(0, "device", "devices", "0 台设备"), "0 devices")
        XCTAssertEqual(en.counted(2, "device", "devices", "2 台设备"), "2 devices")
    }

    /// Chinese has no plural, so its half is passed whole and never branches.
    func testTheChineseHalfIsRenderedExactlyAsItWasWritten() {
        let zh = Translator(language: .zh)

        XCTAssertEqual(zh.counted(1, "device", "devices", "1 台设备"), "1 台设备")
        XCTAssertEqual(zh.counted(9, "device", "devices", "9 台设备"), "9 台设备")
    }

    /// The core's counts arrive unsigned and of several widths; converting them
    /// at the call site is where a truncation would hide.
    func testAnUnsignedCountIsCountedWithoutBeingConvertedFirst() {
        let en = Translator(language: .en)
        let one: UInt32 = 1
        let many: UInt64 = 12

        XCTAssertEqual(en.counted(one, "piece", "pieces", "1 枚"), "1 piece")
        XCTAssertEqual(en.counted(many, "piece", "pieces", "12 枚"), "12 pieces")
    }

    // MARK: Helpers

    private func components(_ color: Color, _ traits: UITraitCollection) -> (
        r: CGFloat, g: CGFloat, b: CGFloat, a: CGFloat
    ) {
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        UIColor(color).resolvedColor(with: traits).getRed(&r, green: &g, blue: &b, alpha: &a)
        return (r, g, b, a)
    }

    private func hex(_ color: Color, _ traits: UITraitCollection) -> Int {
        let (r, g, b, _) = components(color, traits)
        return Int(round(r * 255)) << 16 | Int(round(g * 255)) << 8 | Int(round(b * 255))
    }

    private func alpha(_ color: Color, _ traits: UITraitCollection) -> CGFloat {
        components(color, traits).a
    }

    private func luminance(_ color: Color, _ traits: UITraitCollection) -> CGFloat {
        let (r, g, b, _) = components(color, traits)
        return 0.2126 * r + 0.7152 * g + 0.0722 * b
    }
}
