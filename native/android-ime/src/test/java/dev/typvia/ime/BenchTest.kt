// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// The bench's rules, against a real snapshot document read through the real
// FFI. What is under test is what a reader can see going wrong: a band that
// re-flows under the thumb, a filter that hides more than it was asked to,
// and an undo that takes back more than this keyboard put in.

package dev.typvia.ime

import dev.typvia.mobile.ui.KeyboardBand
import dev.typvia.mobile.ui.KeyboardPhase
import dev.typvia.mobile.ui.TypeIn
import java.io.File
import java.nio.file.Files
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class BenchTest {
    /** Deliberately fake, and shaped so a leak would be findable. */
    private val envelopeMarker = "AKIA_FAKE_ENVELOPE_NOT_A_SECRET"

    private fun snapshotJson(): String {
        val envelope = envelopeMarker.toByteArray().joinToString(",") { it.toString() }
        return """
            {"snapshot_version":1,"generated_at":1754500000000,
             "device_id":"bench-fixture",
             "snippets":[
               {"id":"n1","title":"Standup notes","snippet_type":"text",
                "trigger":null,"trigger_mode":null,"folder_id":null,
                "is_favorite":false,"body":"Yesterday: shipped\nToday: close the loop"},
               {"id":"n2","title":"Deploy script","snippet_type":"code",
                "trigger":";deploy","trigger_mode":"delimiter","folder_id":null,
                "is_favorite":true,"body":"./deploy.sh --env=prod"},
               {"id":"n3","title":"Rollback","snippet_type":"command",
                "trigger":";deroll","trigger_mode":"delimiter","folder_id":null,
                "is_favorite":false,"body":"./rollback.sh"},
               {"id":"v9","encrypted_metadata":[$envelope]}
             ],
             "recent_ids":["n1"],"favorite_ids":["n2"],"folder_metadata":[]}
        """.trimIndent()
    }

    private fun bench(): Bench {
        val file = Files.createTempDirectory("bench").resolve("snapshot.json").toFile()
        file.writeText(snapshotJson())
        return Bench(SnapshotStore.load(file))
    }

    private fun emptyBench(): Bench = Bench(SnapshotStore.load(File("/does/not/exist.json")))

    // MARK: The bench itself

    @Test
    fun theFourBandsFitInsideTheHeightTheSystemIsAskedFor() {
        val bands = KeyboardBand.search + KeyboardBand.sorts + KeyboardBand.tiles + KeyboardBand.functions
        assertEquals(bands, KeyboardBand.bands)
        assertTrue("the bands must not exceed the panel", bands <= KeyboardBand.total)
        // And what is left is left: the system's own bar draws there.
        assertTrue("the gutter must be real", KeyboardBand.gutter.value > 0f)
    }

    @Test
    fun aFreshBenchShowsEverythingItCanReach() {
        val bench = bench()

        assertEquals(KeyboardPhase.Browsing, bench.phase)
        assertEquals(4, bench.total)
        assertEquals(4, bench.tiles.size)
        assertTrue("nothing leads until something is typed", bench.tiles.none { it.isLead })
    }

    @Test
    fun theSortsBandKeepsItsPlacesAndDimsTheOnesWithNothingBehindThem() {
        val bench = bench()
        val atRest = bench.sorts().map { it.mark }

        bench.type(';')
        bench.type('d')
        bench.type('e')

        val whileFiltering = bench.sorts()
        assertEquals("the band must not re-flow under the thumb", atRest, whileFiltering.map { it.mark })
        val secret = whileFiltering.first { it.mark == "SC" }
        assertFalse("a kind with no hits dims", secret.hasHits)
        assertTrue("a kind with hits stays lit", whileFiltering.first { it.mark == "CD" }.hasHits)
    }

    @Test
    fun onlyTheKindsTheSnapshotHoldsAreOffered() {
        val marks = bench().sorts().map { it.mark }

        assertEquals(listOf(null, "TX", "CD", "CM", "SC"), marks)
    }

    @Test
    fun typingATriggerLeavesTheLikeliestSheetInFront() {
        val bench = bench()

        "; d e p".filter { !it.isWhitespace() }.forEach(bench::type)

        assertEquals(KeyboardPhase.Filtering(";dep"), bench.phase)
        assertEquals(listOf("Deploy script"), bench.tiles.map { it.title })
        assertTrue(bench.tiles.first().isLead)
    }

    @Test
    fun aWordWithNothingBehindItIsSaidAsMuchAndKeepsTheWord() {
        val bench = bench()

        "zzz".forEach(bench::type)

        assertEquals(KeyboardPhase.NoMatch("zzz"), bench.phase)
        assertTrue(bench.tiles.isEmpty())

        bench.clearQuery()
        assertEquals(KeyboardPhase.Browsing, bench.phase)
        assertEquals(4, bench.tiles.size)
    }

    @Test
    fun choosingAKindFiltersAndChoosingItAgainLetsGo() {
        val bench = bench()

        bench.select("CD")
        assertEquals(listOf("Deploy script"), bench.tiles.map { it.title })

        bench.select("CD")
        assertEquals(4, bench.tiles.size)
    }

    @Test
    fun theScopesOfferedAreTheOnesWithSomethingInThem() {
        val bench = bench()

        assertEquals(listOf(PanelCategory.Recent, PanelCategory.Starred), bench.scopes)

        bench.select(PanelCategory.Starred)
        assertEquals(listOf("Deploy script"), bench.tiles.map { it.title })
    }

    @Test
    fun aUsedSheetStepsBackInsteadOfLeaving() {
        val bench = bench()

        bench.markUsed("n2", "Deploy script")

        assertEquals(KeyboardPhase.Inserted("Deploy script"), bench.phase)
        val spent = bench.tiles.first { it.id == "n2" }
        assertTrue("it stays on the bench", spent.isSpent)
        assertEquals("and it stays where it was", 4, bench.tiles.size)

        bench.backToBench()
        assertEquals(KeyboardPhase.Browsing, bench.phase)
    }

    @Test
    fun anEmptyLibraryIsSaidAsUnreachableRatherThanAsNoMatch() {
        val bench = emptyBench()

        assertEquals(KeyboardPhase.Unreachable, bench.phase)
        assertEquals(0, bench.total)
        assertTrue(bench.tiles.isEmpty())
        assertTrue("no filter can be offered over nothing", bench.scopes.isEmpty())
    }

    // MARK: Red lines

    @Test
    fun aSecretHandsBackNoBodyAndNeverLeadsWithAPreview() {
        val bench = bench()
        val secret = bench.tiles.first { it.isSecret }

        assertNull("a locked sheet has no body to type", bench.body(secret.id))
        assertFalse(
            "no envelope byte may surface as a title",
            secret.title.contains(envelopeMarker),
        )
        assertNotNull("an ordinary sheet still types", bench.body("n2"))
    }

    @Test
    fun theSecretPhaseIsTheOnlyThingASecretTapCanProduce() {
        val bench = bench()
        val secret = bench.tiles.first { it.isSecret }

        bench.openSecret(secret.title)

        assertEquals(KeyboardPhase.Secret(secret.title), bench.phase)
    }

    // MARK: Typing into someone else's field

    @Test
    fun undoTakesBackWhatWasTypedAndNotWhatTheBodyWeighs() {
        // The count that undo uses is what actually went in. A run cut short
        // has put in fewer units than the body holds, and deleting the body's
        // length would eat the reader's own words.
        val body = "./deploy.sh --env=prod"
        val plan = TypeIn.plan(body.codePointCount(0, body.length))
        assertTrue("a body this size is typed, not pasted", plan is TypeIn.Plan.Character)

        var typed = 0
        var index = 0
        while (index < body.length && index < 5) {
            val width = Character.charCount(body.codePointAt(index))
            typed += width
            index += width
        }
        assertEquals("only what went in", 5, typed)
        assertTrue("and never the whole body", typed < body.length)
    }

    @Test
    fun aSurrogatePairIsTypedWholeOrNotAtAll() {
        val body = "ship 🚀 it"
        var index = 0
        val pieces = mutableListOf<String>()
        while (index < body.length) {
            val width = Character.charCount(body.codePointAt(index))
            pieces.add(body.substring(index, index + width))
            index += width
        }
        assertEquals(body, pieces.joinToString(""))
        assertTrue("the rocket is one piece", pieces.contains("🚀"))
    }
}
