// Share save sheet: a full-width bottom sheet over the dimmed host — grabber,
// accent-bar header, the shared text in a sunken mono box, a TITLE field on
// a hairline rule, and an ink-filled Save plate beside a Cancel text action.
// The design's type tabs (Text/Code/Prompt/Secret) and the sensitive-notice
// row are omitted: inbox schema v1 carries no type or security field and the
// host ingests every share as a plain text snippet, so the sheet offers no
// choice it cannot honor. Save lands the document via ShareInbox; behavior
// (states, limits, honest feedback) is unchanged from the previous card.
// Native views only, built from ShareTheme tokens; no icons on this surface,
// no third-party UI.

package dev.typvia.share

import android.app.Activity
import android.content.Intent
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.os.Build
import android.os.Bundle
import android.text.InputType
import android.text.method.ScrollingMovementMethod
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.view.WindowInsets
import android.view.inputmethod.EditorInfo
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.TextView

class ShareTargetActivity : Activity() {
    private lateinit var tokens: ShareTheme
    private var sharedText = ""

    private lateinit var saveButton: TextView
    private lateinit var preview: TextView
    private lateinit var metaLabel: TextView
    private lateinit var titleField: EditText
    private lateinit var errorNotice: View

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        tokens = ShareTheme(this)
        setContentView(buildLayout())
        // The share payload is external input: a wrong action/type, a
        // missing text extra, blank text or an over-limit payload degrades
        // to an honest refusing state with Cancel as the only action
        // (business-error handling, never a crash). The state derivation
        // itself is pure (ShareCardState.kt) and unit-tested.
        val text = extractSharedText(intent)
        val state = shareCardState(text)
        sharedText = text ?: ""
        preview.text = sharedText
        titleField.hint = if (text == null) "" else ShareInbox.derivedTitle(text)
        metaLabel.text = state.metaLine
        setSaveEnabled(state.saveEnabled)
    }

    // MARK: Shared-item loading

    /** The only accepted payload: ACTION_SEND with a text/plain-typed
     * CharSequence EXTRA_TEXT. Everything else reads as "no text". */
    private fun extractSharedText(intent: Intent?): String? {
        if (intent?.action != Intent.ACTION_SEND) return null
        if (intent.type?.startsWith("text/") != true) return null
        return intent.getCharSequenceExtra(Intent.EXTRA_TEXT)?.toString()
    }

    private fun setSaveEnabled(enabled: Boolean) {
        saveButton.isEnabled = enabled
        saveButton.alpha = if (enabled) 1f else tokens.disabledAlpha
    }

    // MARK: Actions

    private fun saveTapped() {
        errorNotice.visibility = View.GONE
        val typed = titleField.text.toString().trim()
        val saved = ShareInbox.write(
            root = dataDir,
            sharedAt = System.currentTimeMillis(),
            // An empty field means "no title sent": the host owns the
            // first-line derivation (single source of truth for the rule).
            title = typed.ifEmpty { null },
            text = sharedText,
        )
        if (saved) {
            finish()
        } else {
            // What is still good, first (design failure-state rule); the
            // message never echoes the shared content.
            errorNotice.visibility = View.VISIBLE
        }
    }

    // MARK: Layout

    private fun buildLayout(): View {
        val root = FrameLayout(this)

        val sheet = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            background = GradientDrawable().apply {
                setColor(tokens.sheet)
                cornerRadii = floatArrayOf(
                    tokens.sheetCorner, tokens.sheetCorner,
                    tokens.sheetCorner, tokens.sheetCorner,
                    0f, 0f, 0f, 0f,
                )
            }
            setPadding(tokens.sidePad, tokens.topPad, tokens.sidePad, tokens.sheetBottomResting)
        }

        val grabber = View(this).apply {
            background = GradientDrawable().apply {
                setColor(tokens.grabber)
                cornerRadius = tokens.grabberHeight / 2f
            }
        }
        sheet.addView(
            grabber,
            LinearLayout.LayoutParams(tokens.grabberWidth, tokens.grabberHeight).apply {
                gravity = Gravity.CENTER_HORIZONTAL
            },
        )

        sheet.addView(buildHeader(), matchWidth(topMargin = tokens.afterGrabber))

        preview = TextView(this).apply {
            typeface = Typeface.MONOSPACE
            setTextSize(TypedValue.COMPLEX_UNIT_SP, tokens.previewSp)
            // The mono box's fixed 13/1.7 line grid from the mock.
            lineHeight = tokens.previewLineHeight
            setTextColor(tokens.codeInk)
            background = GradientDrawable().apply {
                setColor(tokens.sunken)
                cornerRadius = tokens.previewCorner
            }
            setPadding(tokens.previewPad, tokens.previewPad, tokens.previewPad, tokens.previewPad)
            minHeight = tokens.previewLineHeight + 2 * tokens.previewPad
            maxHeight = tokens.previewMaxHeight
            movementMethod = ScrollingMovementMethod()
            isVerticalScrollBarEnabled = true
            contentDescription = "Shared text"
        }
        sheet.addView(preview, matchWidth(topMargin = tokens.afterHeader))

        metaLabel = TextView(this).apply {
            setTextSize(TypedValue.COMPLEX_UNIT_SP, tokens.metaSp)
            setTextColor(tokens.meta)
        }
        sheet.addView(metaLabel, matchWidth(topMargin = tokens.afterPreview))

        val fieldLabel = TextView(this).apply {
            text = "TITLE"
            typeface = Typeface.MONOSPACE
            setTextSize(TypedValue.COMPLEX_UNIT_SP, tokens.fieldLabelSp)
            letterSpacing = tokens.fieldLabelTracking
            setTextColor(tokens.metaMono)
        }
        sheet.addView(fieldLabel, matchWidth(topMargin = tokens.afterMeta))

        titleField = EditText(this).apply {
            background = null
            setPadding(0, 0, 0, 0)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, tokens.fieldSp)
            letterSpacing = tokens.fieldTracking
            setTextColor(tokens.ink)
            setHintTextColor(tokens.meta)
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_CAP_SENTENCES
            imeOptions = EditorInfo.IME_ACTION_DONE
            maxLines = 1
            contentDescription = "Title"
        }
        sheet.addView(titleField, matchWidth(topMargin = tokens.afterFieldLabel))

        // The field's 1px hairline underline — the design's field language
        // (never a boxed input border).
        val rule = View(this).apply { setBackgroundColor(tokens.hairline) }
        sheet.addView(
            rule,
            matchWidth(topMargin = tokens.afterField).apply { height = tokens.ruleHeight },
        )

        errorNotice = buildErrorNotice()
        sheet.addView(errorNotice, matchWidth(topMargin = tokens.blockGap))

        sheet.addView(buildActions(), matchWidth(topMargin = tokens.blockGap))

        root.addView(sheet, FrameLayout.LayoutParams(MATCH, WRAP, Gravity.BOTTOM))

        // Transparent bars + translucent window mean the root spans the full
        // screen: the sheet keeps its resting 40dp action clearance, growing
        // it when the navigation/IME inset needs more, so the actions sit
        // above the bar and ride up with the keyboard.
        root.setOnApplyWindowInsetsListener { _, insets ->
            val inset = bottomInset(insets)
            sheet.setPadding(
                tokens.sidePad,
                tokens.topPad,
                tokens.sidePad,
                maxOf(tokens.sheetBottomResting, inset + tokens.sheetBottomAboveInset),
            )
            insets
        }
        return root
    }

    /** Header row: the 2×11 accent caret bar and the uppercase wordmark. */
    private fun buildHeader(): View =
        LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            addView(
                View(this@ShareTargetActivity).apply { setBackgroundColor(tokens.accent) },
                LinearLayout.LayoutParams(tokens.headerBarWidth, tokens.headerBarHeight),
            )
            addView(
                TextView(this@ShareTargetActivity).apply {
                    text = "SAVE TO TYPVIA"
                    typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
                    setTextSize(TypedValue.COMPLEX_UNIT_SP, tokens.headerSp)
                    letterSpacing = tokens.headerTracking
                    setTextColor(tokens.headerInk)
                    contentDescription = "Save to Typvia"
                },
                LinearLayout.LayoutParams(WRAP, WRAP).apply {
                    marginStart = tokens.headerGap
                },
            )
        }

    /** Write-failure notice in the sheet's statement language: accent bar,
     * ink first line, secondary detail. Hidden until a save fails. */
    private fun buildErrorNotice(): View =
        LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            visibility = View.GONE
            addView(
                View(this@ShareTargetActivity).apply { setBackgroundColor(tokens.accent) },
                LinearLayout.LayoutParams(tokens.noticeBarWidth, tokens.noticeBarHeight),
            )
            val lines = LinearLayout(this@ShareTargetActivity).apply {
                orientation = LinearLayout.VERTICAL
                addView(
                    TextView(this@ShareTargetActivity).apply {
                        text = "Your text is untouched."
                        setTextSize(TypedValue.COMPLEX_UNIT_SP, tokens.noticeSp)
                        setTextColor(tokens.ink)
                    },
                    LinearLayout.LayoutParams(MATCH, WRAP),
                )
                addView(
                    TextView(this@ShareTargetActivity).apply {
                        text = "It could not be handed to Typvia. Try again."
                        setTextSize(TypedValue.COMPLEX_UNIT_SP, tokens.noticeDetailSp)
                        setTextColor(tokens.secondary)
                    },
                    LinearLayout.LayoutParams(MATCH, WRAP).apply {
                        topMargin = tokens.noticeLineGap
                    },
                )
            }
            addView(
                lines,
                LinearLayout.LayoutParams(MATCH, WRAP).apply {
                    marginStart = tokens.noticeGap
                },
            )
        }

    /** Bottom action row: the ink-filled Save plate and the Cancel text
     * action (never a second filled button). */
    private fun buildActions(): View =
        LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            saveButton = TextView(this@ShareTargetActivity).apply {
                text = "Save"
                typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
                setTextSize(TypedValue.COMPLEX_UNIT_SP, tokens.buttonSp)
                setTextColor(tokens.buttonLabel)
                gravity = Gravity.CENTER
                background = GradientDrawable().apply {
                    setColor(tokens.buttonFill)
                    cornerRadius = tokens.buttonCorner
                }
                setOnClickListener { saveTapped() }
            }
            addView(saveButton, LinearLayout.LayoutParams(0, tokens.buttonHeight, 1f))
            addView(
                TextView(this@ShareTargetActivity).apply {
                    text = "Cancel"
                    setTextSize(TypedValue.COMPLEX_UNIT_SP, tokens.cancelSp)
                    setTextColor(tokens.secondary)
                    gravity = Gravity.CENTER
                    // Comfortable tap target for a chrome-less text action.
                    minWidth = tokens.minTapTarget
                    minHeight = tokens.minTapTarget
                    setOnClickListener { finish() }
                },
                LinearLayout.LayoutParams(WRAP, WRAP).apply {
                    marginStart = tokens.actionGap
                },
            )
        }

    private fun bottomInset(insets: WindowInsets): Int =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            insets.getInsets(WindowInsets.Type.ime() or WindowInsets.Type.systemBars()).bottom
        } else {
            // Pre-R spelling of the same value (deprecated in R, valid on
            // the API 28/29 devices this branch serves).
            @Suppress("DEPRECATION")
            insets.systemWindowInsetBottom
        }

    private fun matchWidth(topMargin: Int = 0): LinearLayout.LayoutParams =
        LinearLayout.LayoutParams(MATCH, WRAP).apply { this.topMargin = topMargin }

    private companion object {
        const val MATCH = LinearLayout.LayoutParams.MATCH_PARENT
        const val WRAP = LinearLayout.LayoutParams.WRAP_CONTENT
    }
}
