// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// Instrumentation closed loop for the Typvia IME, mirroring the
// iOS XCUITest (native/ios-keyboard-ext/UITests): enable the IME, switch to
// it, tap a snippet row inside a real host field, and assert the insertion
// plus the two red lines (locked rows insert nothing, password fields refuse
// everything).
//
// The loop runs entirely inside the self-instrumenting test APK
// (dev.typvia.ime.test): the fixture snapshot is written into that package's
// own data dir — the same same-UID read the product relies on — and the
// installed dev.typvia.mobile app and its data are never touched. UiAutomator
// does the driving because the IME renders in its own window and process
// (":ime"), which an Espresso view scope cannot reach.

package dev.typvia.ime

import android.content.Context
import android.os.SystemClock
import androidx.test.core.app.ActivityScenario
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import java.io.File
import org.junit.After
import org.junit.AfterClass
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class KeyboardClosedLoopTest {
    private lateinit var scenario: ActivityScenario<ImeHostActivity>

    @Before
    fun launchHost() {
        scenario = ActivityScenario.launch(ImeHostActivity::class.java)
    }

    @After
    fun closeHost() {
        scenario.close()
    }

    // MARK: The loop

    @Test
    fun tappingASnippetRowInsertsItsFullBody() {
        tap(ImeHostActivity.PLAIN_FIELD)
        // The strip rendering at all closes the read side of the loop: its
        // title can only have come from the fixture snapshot on disk.
        tap(STANDUP_STRIP)
        awaitPlainText(STANDUP_BODY)
    }

    @Test
    fun lockedRowsShowNoPlaintextAndInsertNothing() {
        tap(ImeHostActivity.PLAIN_FIELD)
        tap(STANDUP_STRIP)
        awaitPlainText(STANDUP_BODY)

        expandPanel()
        val locked = device.wait(Until.findObject(By.desc(LOCKED_DESC)), FIND_TIMEOUT_MS)
        assertNotNull("locked row must render in the expanded panel", locked)
        // The locked row carries the generic label and nothing else — the
        // fixture's envelope bytes must never surface as text anywhere.
        assertNotNull(device.findObject(By.text(PanelStrings.LOCKED_TITLE)))
        assertNull(device.findObject(By.textContains(ENVELOPE_MARKER)))

        locked.click()
        // Asserting the *absence* of an insertion needs a bounded settle
        // window; the insert path lands well under this on the emulator.
        SystemClock.sleep(SETTLE_MS)
        assertEquals("locked row must not insert", STANDUP_BODY, plainText())
    }

    @Test
    fun passwordFieldsRefuseTheWholePanel() {
        tap(ImeHostActivity.PASSWORD_FIELD)
        // The service keeps its bar/panel expansion across input sessions,
        // so either refusal face may greet the field first; wait for one,
        // then walk both states so each is asserted regardless of order.
        awaitAnyRefusal()

        collapsePanel()
        assertNotNull(
            "password refusal bar must show",
            device.wait(Until.findObject(By.textContains(PASSWORD_BAR_MATCH)), FIND_TIMEOUT_MS),
        )
        assertNull(device.findObject(By.desc(STANDUP_STRIP)))

        expandPanel()
        assertNotNull(
            "password refusal state must show in the panel",
            device.wait(Until.findObject(By.text(PanelStrings.PASSWORD_MAIN)), FIND_TIMEOUT_MS),
        )
        assertNull(device.findObject(By.desc(STANDUP_STRIP)))
        assertNull(device.findObject(By.desc(DOCKER_STRIP)))

        SystemClock.sleep(SETTLE_MS)
        assertEquals("nothing may enter a password field", "", passwordText())
    }

    // MARK: Helpers

    /** Waits for an element by accessibility description, then taps it. */
    private fun tap(desc: String) {
        val target = device.wait(Until.findObject(By.desc(desc)), FIND_TIMEOUT_MS)
        assertNotNull("expected on-screen element: $desc", target)
        target.click()
    }

    /** Brings the expanded panel up whatever state the service kept. */
    private fun expandPanel() {
        device.wait(Until.findObject(By.desc(EXPAND_DESC)), SHORT_TIMEOUT_MS)?.click()
        assertNotNull(
            "expanded panel must show its collapse affordance",
            device.wait(Until.findObject(By.desc(COLLAPSE_DESC)), FIND_TIMEOUT_MS),
        )
    }

    /** Brings the bar back whatever state the service kept. */
    private fun collapsePanel() {
        device.wait(Until.findObject(By.desc(COLLAPSE_DESC)), SHORT_TIMEOUT_MS)?.click()
        assertNotNull(
            "bar must show its expand affordance",
            device.wait(Until.findObject(By.desc(EXPAND_DESC)), FIND_TIMEOUT_MS),
        )
    }

    /** Waits until either refusal face (bar copy or panel state) is up. */
    private fun awaitAnyRefusal() {
        val deadline = SystemClock.uptimeMillis() + FIND_TIMEOUT_MS
        while (SystemClock.uptimeMillis() < deadline) {
            if (device.hasObject(By.textContains(PASSWORD_BAR_MATCH)) ||
                device.hasObject(By.text(PanelStrings.PASSWORD_MAIN))
            ) {
                return
            }
            SystemClock.sleep(POLL_MS)
        }
        fail("no password refusal state appeared")
    }

    private fun plainText(): String {
        var text = ""
        scenario.onActivity { text = it.plainField.text.toString() }
        return text
    }

    private fun passwordText(): String {
        var text = ""
        scenario.onActivity { text = it.passwordField.text.toString() }
        return text
    }

    /** Polls the real EditText (same process) until the expected body lands. */
    private fun awaitPlainText(expected: String) {
        val deadline = SystemClock.uptimeMillis() + FIND_TIMEOUT_MS
        while (SystemClock.uptimeMillis() < deadline) {
            if (plainText() == expected) return
            SystemClock.sleep(POLL_MS)
        }
        assertEquals(expected, plainText())
    }

    companion object {
        private const val SERVICE = "dev.typvia.ime.TypviaImeService"
        private const val EXPAND_DESC = "Expand snippet panel"
        private const val COLLAPSE_DESC = "Collapse to snippet bar"
        private const val LOCKED_DESC = "Locked, vault snippet"
        private const val STANDUP_STRIP = "text, Standup notes"
        private const val DOCKER_STRIP = "command, Docker logs"
        /** Multiline on purpose: exercises commitText across line breaks. */
        private const val STANDUP_BODY =
            "Yesterday: shipped the IME panel\nToday: close the loop\nNo blockers"
        /** Substring of PanelStrings.PASSWORD_BAR stable across copy tweaks. */
        private const val PASSWORD_BAR_MATCH = "password fields"
        /** Deliberately fake marker inside the sensitive
         * envelope, so leak assertions can grep the screen for it. */
        private const val ENVELOPE_MARKER = "AKIA_FAKE_ENVELOPE"

        private const val FIND_TIMEOUT_MS = 10_000L
        private const val SHORT_TIMEOUT_MS = 3_000L
        private const val SETTLE_MS = 800L
        private const val POLL_MS = 100L
        private const val IME_REGISTER_TIMEOUT_MS = 30_000L
        private const val IME_RETRY_MS = 500L

        private lateinit var device: UiDevice
        private lateinit var fixtureFile: File
        private var previousIme = ""

        private fun context(): Context = ApplicationProvider.getApplicationContext()

        private fun imeId(): String = "${context().packageName}/$SERVICE"

        private fun shell(command: String): String =
            device.executeShellCommand(command)

        /** Contract-valid snapshot: two normal
         * entries (one recent, one triggered) and one sensitive entry that is
         * nothing but an id plus an opaque envelope. */
        private fun fixtureJson(): String {
            val body = STANDUP_BODY.replace("\n", "\\n")
            val envelope = ENVELOPE_MARKER.toByteArray().joinToString(",") { it.toString() }
            return """
                {"snapshot_version":1,"generated_at":1754500000000,
                 "device_id":"instrumentation-fixture",
                 "snippets":[
                   {"id":"n1","title":"Standup notes","snippet_type":"text",
                    "trigger":null,"trigger_mode":null,"folder_id":null,
                    "is_favorite":false,"body":"$body"},
                   {"id":"n2","title":"Docker logs","snippet_type":"command",
                    "trigger":":dlog","trigger_mode":"immediate","folder_id":null,
                    "is_favorite":false,"body":"docker logs -f app"},
                   {"id":"v9","encrypted_metadata":[$envelope]}
                 ],
                 "recent_ids":["n1"],"favorite_ids":[],"folder_metadata":[]}
            """.trimIndent()
        }

        /** Writes the fixture into the test package's own data dir (the same
         * same-UID read path the product uses) and makes the Typvia IME the
         * active input method. `ime enable` needs a retry loop: registration
         * lags the install by a few seconds. */
        @BeforeClass
        @JvmStatic
        fun activateImeWithFixture() {
            device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
            fixtureFile = File(context().dataDir, SnapshotStore.RELATIVE_PATH)
            fixtureFile.parentFile?.mkdirs()
            fixtureFile.writeText(fixtureJson())

            shell("input keyevent KEYCODE_WAKEUP")
            shell("wm dismiss-keyguard")

            previousIme = shell("settings get secure default_input_method").trim()
            var registered = false
            val deadline = SystemClock.uptimeMillis() + IME_REGISTER_TIMEOUT_MS
            while (SystemClock.uptimeMillis() < deadline) {
                shell("ime enable ${imeId()}")
                if (shell("ime list -s").contains(imeId())) {
                    registered = true
                    break
                }
                SystemClock.sleep(IME_RETRY_MS)
            }
            assertTrue("IME never registered: ${imeId()}", registered)

            shell("ime set ${imeId()}")
            val selectDeadline = SystemClock.uptimeMillis() + IME_REGISTER_TIMEOUT_MS
            while (SystemClock.uptimeMillis() < selectDeadline) {
                if (shell("settings get secure default_input_method").trim() == imeId()) return
                SystemClock.sleep(IME_RETRY_MS)
            }
            assertEquals(imeId(), shell("settings get secure default_input_method").trim())
        }

        /** Restores the previous IME and removes the fixture — the loop must
         * leave the emulator exactly as it found it. */
        @AfterClass
        @JvmStatic
        fun restoreImeAndRemoveFixture() {
            if (previousIme.isNotEmpty() && previousIme != "null" && previousIme != imeId()) {
                shell("ime set $previousIme")
            } else {
                shell("ime reset")
            }
            shell("ime disable ${imeId()}")
            fixtureFile.delete()
            fixtureFile.parentFile?.delete()
        }
    }
}
