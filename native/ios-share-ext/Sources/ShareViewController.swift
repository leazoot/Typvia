// Share save sheet: a full-width bottom sheet over the dimmed host — grabber,
// accent-bar header, the shared text in a sunken mono box, a TITLE field on
// a hairline rule, and an ink-filled Save plate beside a Cancel text action.
// The design's type tabs (Text/Code/Prompt/Secret) and the sensitive-notice
// row are omitted: inbox schema v1 carries no type or security field and the
// host ingests every share as a plain text snippet, so the sheet offers no
// choice it cannot honor. Save lands the document via ShareInbox; behavior
// (states, limits, honest feedback) is unchanged from the previous card.

import UIKit
import UniformTypeIdentifiers

final class ShareViewController: UIViewController {
    private var sharedText = ""

    private let sheet = UIView()
    private let grabber = UIView()
    private let saveButton = UIButton(type: .custom)
    private let cancelButton = UIButton(type: .system)
    private let preview = UITextView()
    private let metaLabel = UILabel()
    private let titleLabel = UILabel()
    private let titleField = UITextField()
    private let titleRule = UIView()
    private let errorNotice = UIStackView()

    override func viewDidLoad() {
        super.viewDidLoad()
        buildLayout()
        showLoading()
        loadSharedText()
    }

    // MARK: Shared-item loading

    /// Pulls the first plain-text attachment. The activation rule limits the
    /// sheet to text shares, but the payload is still external input: a
    /// missing or non-decodable item degrades to an honest empty state with
    /// Cancel as the only action.
    private func loadSharedText() {
        let providers = (extensionContext?.inputItems as? [NSExtensionItem])?
            .compactMap { $0.attachments }
            .flatMap { $0 } ?? []
        let typeId = UTType.plainText.identifier
        guard let provider = providers.first(where: {
            $0.hasItemConformingToTypeIdentifier(typeId)
        }) else {
            showUnavailable()
            return
        }
        provider.loadItem(forTypeIdentifier: typeId, options: nil) { [weak self] item, _ in
            DispatchQueue.main.async {
                guard let self else { return }
                if let text = item as? String {
                    self.present(text: text)
                } else if let data = item as? Data, let text = String(data: data, encoding: .utf8) {
                    self.present(text: text)
                } else {
                    self.showUnavailable()
                }
            }
        }
    }

    // MARK: States

    private func showLoading() {
        setPreview(text: "")
        metaLabel.text = "Loading the shared text…"
        setSaveEnabled(false)
    }

    private func present(text: String) {
        sharedText = text
        setPreview(text: text)
        titleField.attributedPlaceholder = NSAttributedString(
            string: ShareInbox.derivedTitle(from: text),
            attributes: [
                .font: ShareTheme.fieldFont,
                .kern: ShareTheme.fieldKern,
                .foregroundColor: ShareTheme.meta,
            ])
        let count = text.count
        let counted = "\(count) character\(count == 1 ? "" : "s")"
        let blank = text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        let oversize = text.utf8.count > ShareInbox.textMaxBytes
        if blank {
            metaLabel.text = "Nothing to save — the share holds no text"
        } else if oversize {
            metaLabel.text = "\(counted) · over the 128 KB limit, too long to save"
        } else {
            metaLabel.text = counted
        }
        setSaveEnabled(!blank && !oversize)
    }

    private func showUnavailable() {
        sharedText = ""
        setPreview(text: "")
        metaLabel.text = "Nothing to save — this share did not include text"
        setSaveEnabled(false)
    }

    private func setSaveEnabled(_ enabled: Bool) {
        saveButton.isEnabled = enabled
        saveButton.alpha = enabled ? 1 : ShareTheme.disabledAlpha
    }

    // MARK: Actions

    @objc private func cancelTapped() {
        extensionContext?.cancelRequest(withError: CocoaError(.userCancelled))
    }

    @objc private func saveTapped() {
        errorNotice.isHidden = true
        let typed = (titleField.text ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        let document = ShareInboxDocument(
            sharedAt: Int64(Date().timeIntervalSince1970 * 1000),
            // An empty field means "no title sent": the host owns the
            // first-line derivation (single source of truth for the rule).
            title: typed.isEmpty ? nil : typed,
            text: sharedText)
        do {
            try ShareInbox.write(document)
            extensionContext?.completeRequest(returningItems: nil)
        } catch {
            // What is still good, first (design failure-state rule); the
            // message never echoes the shared content.
            errorNotice.isHidden = false
        }
    }

    // MARK: Layout

    private func buildLayout() {
        view.backgroundColor = ShareTheme.dim

        sheet.backgroundColor = ShareTheme.sheet
        sheet.layer.cornerRadius = ShareTheme.sheetCorner
        sheet.layer.maskedCorners = [.layerMinXMinYCorner, .layerMaxXMinYCorner]
        sheet.layer.shadowColor = ShareTheme.shadowColor.cgColor
        sheet.layer.shadowOpacity = ShareTheme.shadowOpacity
        sheet.layer.shadowOffset = ShareTheme.shadowOffset
        sheet.layer.shadowRadius = ShareTheme.shadowRadius
        sheet.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(sheet)

        grabber.backgroundColor = ShareTheme.grabber
        grabber.layer.cornerRadius = ShareTheme.grabberSize.height / 2
        grabber.translatesAutoresizingMaskIntoConstraints = false
        sheet.addSubview(grabber)

        let header = buildHeader()
        buildPreview()

        metaLabel.font = ShareTheme.metaFont
        metaLabel.textColor = ShareTheme.meta

        titleLabel.attributedText = NSAttributedString(
            string: "TITLE",
            attributes: [.kern: ShareTheme.fieldLabelKern])
        titleLabel.font = ShareTheme.fieldLabelFont
        titleLabel.textColor = ShareTheme.metaMono

        titleField.font = ShareTheme.fieldFont
        titleField.defaultTextAttributes[.kern] = ShareTheme.fieldKern
        titleField.textColor = ShareTheme.ink
        titleField.tintColor = ShareTheme.accent
        titleField.autocapitalizationType = .sentences
        titleField.returnKeyType = .done
        titleField.addTarget(self, action: #selector(titleReturn), for: .editingDidEndOnExit)
        titleField.accessibilityLabel = "Title"

        titleRule.backgroundColor = ShareTheme.hairline

        buildErrorNotice()
        let actions = buildActions()

        let column = UIStackView(arrangedSubviews: [
            header, preview, metaLabel, titleLabel, titleField, titleRule,
            errorNotice, actions,
        ])
        column.axis = .vertical
        column.setCustomSpacing(22, after: header)
        column.setCustomSpacing(12, after: preview)
        column.setCustomSpacing(18, after: metaLabel)
        column.setCustomSpacing(10, after: titleLabel)
        column.setCustomSpacing(11, after: titleField)
        column.setCustomSpacing(26, after: titleRule)
        column.setCustomSpacing(26, after: errorNotice)
        column.translatesAutoresizingMaskIntoConstraints = false
        sheet.addSubview(column)

        let side = ShareTheme.sidePad
        // Actions sit 40pt off the sheet's bottom edge; the
        // required constraint keeps them above the keyboard when the title
        // field is being edited (the keyboard guide equals the bottom safe
        // area while hidden, so both agree on home-indicator devices).
        let restingBottom = column.bottomAnchor.constraint(
            equalTo: sheet.bottomAnchor, constant: -40)
        restingBottom.priority = .defaultHigh
        NSLayoutConstraint.activate([
            sheet.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            sheet.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            sheet.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            grabber.topAnchor.constraint(
                equalTo: sheet.topAnchor, constant: ShareTheme.topPad),
            grabber.centerXAnchor.constraint(equalTo: sheet.centerXAnchor),
            grabber.widthAnchor.constraint(equalToConstant: ShareTheme.grabberSize.width),
            grabber.heightAnchor.constraint(equalToConstant: ShareTheme.grabberSize.height),
            column.topAnchor.constraint(equalTo: grabber.bottomAnchor, constant: 24),
            column.leadingAnchor.constraint(equalTo: sheet.leadingAnchor, constant: side),
            column.trailingAnchor.constraint(equalTo: sheet.trailingAnchor, constant: -side),
            column.bottomAnchor.constraint(
                lessThanOrEqualTo: view.keyboardLayoutGuide.topAnchor, constant: -6),
            restingBottom,
            preview.heightAnchor.constraint(
                lessThanOrEqualToConstant: ShareTheme.previewMaxHeight),
            preview.heightAnchor.constraint(
                greaterThanOrEqualToConstant: ShareTheme.previewLineHeight
                    + 2 * ShareTheme.previewPad),
            titleRule.heightAnchor.constraint(equalToConstant: ShareTheme.fieldRuleHeight),
        ])
    }

    /// Header row: the 2×11 accent caret bar and the uppercase wordmark.
    private func buildHeader() -> UIView {
        let bar = UIView()
        bar.backgroundColor = ShareTheme.accent
        bar.translatesAutoresizingMaskIntoConstraints = false
        bar.widthAnchor.constraint(
            equalToConstant: ShareTheme.headerBarSize.width).isActive = true
        bar.heightAnchor.constraint(
            equalToConstant: ShareTheme.headerBarSize.height).isActive = true

        let label = UILabel()
        label.attributedText = NSAttributedString(
            string: "SAVE TO TYPVIA",
            attributes: [.kern: ShareTheme.headerKern])
        label.font = ShareTheme.headerFont
        label.textColor = ShareTheme.headerInk
        label.accessibilityLabel = "Save to Typvia"

        let row = UIStackView(arrangedSubviews: [bar, label])
        row.axis = .horizontal
        row.alignment = .center
        row.spacing = 8
        return row
    }

    private func buildPreview() {
        preview.isEditable = false
        preview.backgroundColor = ShareTheme.sunken
        preview.layer.cornerRadius = ShareTheme.previewCorner
        let pad = ShareTheme.previewPad
        preview.textContainerInset = UIEdgeInsets(top: pad, left: pad, bottom: pad, right: pad)
        preview.textContainer.lineFragmentPadding = 0
        preview.accessibilityLabel = "Shared text"
    }

    /// Mono box content at the design's fixed 22pt line height, wrapped at
    /// any character (secrets and URLs have no word boundaries to break on).
    private func setPreview(text: String) {
        let paragraph = NSMutableParagraphStyle()
        paragraph.minimumLineHeight = ShareTheme.previewLineHeight
        paragraph.maximumLineHeight = ShareTheme.previewLineHeight
        paragraph.lineBreakMode = .byCharWrapping
        preview.attributedText = NSAttributedString(
            string: text,
            attributes: [
                .font: ShareTheme.previewFont,
                .foregroundColor: ShareTheme.codeInk,
                .paragraphStyle: paragraph,
            ])
    }

    /// Write-failure notice in the sheet's statement language: accent bar,
    /// ink first line, secondary detail. Hidden until a save fails.
    private func buildErrorNotice() {
        let bar = UIView()
        bar.backgroundColor = ShareTheme.accent
        bar.translatesAutoresizingMaskIntoConstraints = false
        bar.widthAnchor.constraint(
            equalToConstant: ShareTheme.noticeBarWidth).isActive = true
        bar.heightAnchor.constraint(equalToConstant: 34).isActive = true

        let line = UILabel()
        line.text = "Your text is untouched."
        line.font = ShareTheme.noticeFont
        line.textColor = ShareTheme.ink

        let detail = UILabel()
        detail.text = "It could not be handed to Typvia. Try again."
        detail.font = ShareTheme.noticeDetailFont
        detail.textColor = ShareTheme.secondary
        detail.numberOfLines = 0

        let lines = UIStackView(arrangedSubviews: [line, detail])
        lines.axis = .vertical
        lines.spacing = 3

        errorNotice.addArrangedSubview(bar)
        errorNotice.addArrangedSubview(lines)
        errorNotice.axis = .horizontal
        errorNotice.alignment = .top
        errorNotice.spacing = 9
        errorNotice.isHidden = true
    }

    /// Bottom action row: the ink-filled Save plate and the Cancel text
    /// action (never a second filled button).
    private func buildActions() -> UIView {
        saveButton.setTitle("Save", for: .normal)
        saveButton.titleLabel?.font = ShareTheme.buttonFont
        saveButton.setTitleColor(ShareTheme.buttonLabel, for: .normal)
        saveButton.backgroundColor = ShareTheme.buttonFill
        saveButton.layer.cornerRadius = ShareTheme.buttonCorner
        saveButton.addTarget(self, action: #selector(saveTapped), for: .touchUpInside)
        saveButton.heightAnchor.constraint(
            equalToConstant: ShareTheme.buttonHeight).isActive = true

        cancelButton.setTitle("Cancel", for: .normal)
        cancelButton.titleLabel?.font = ShareTheme.cancelFont
        cancelButton.setTitleColor(ShareTheme.secondary, for: .normal)
        cancelButton.addTarget(self, action: #selector(cancelTapped), for: .touchUpInside)
        // Comfortable tap target for a chrome-less text action.
        cancelButton.widthAnchor.constraint(greaterThanOrEqualToConstant: 44).isActive = true

        let row = UIStackView(arrangedSubviews: [saveButton, cancelButton])
        row.axis = .horizontal
        row.alignment = .center
        row.spacing = 18
        return row
    }

    @objc private func titleReturn() {
        titleField.resignFirstResponder()
    }
}
