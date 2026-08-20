// View pieces of the keyboard panel: the fixed five-tab bar with the ABC
// key, title+preview snippet rows, locked vault rows (dot string + LOCKED,
// zero plaintext), the template fill face, the vault locked figure, the
// inserted hint, and the text-built empty state. No icons, no spinners, no
// illustrations.

import UIKit

// MARK: - Tabs

/// The five fixed panel sections of the keyboard tab bar.
enum PanelTab: CaseIterable {
    case recent
    case favorites
    case folders
    case templates
    case vault

    var label: String {
        switch self {
        case .recent: return "Recent"
        case .favorites: return "Favorites"
        case .folders: return "Folders"
        case .templates: return "Templates"
        case .vault: return "Vault"
        }
    }
}

// MARK: - Tab bar

/// One tab: label centered on the bar, a 2px accent indicator floating
/// above it when active. Selection is the indicator + ink weight, never a
/// fill.
private final class TabItemView: UIControl {
    let tab: PanelTab
    private let indicator = UIView()
    private let label = UILabel()

    init(tab: PanelTab) {
        self.tab = tab
        super.init(frame: .zero)
        indicator.backgroundColor = PanelTheme.accent
        label.text = tab.label
        for view in [indicator, label] {
            view.translatesAutoresizingMaskIntoConstraints = false
            view.isUserInteractionEnabled = false
            addSubview(view)
        }
        NSLayoutConstraint.activate([
            label.leadingAnchor.constraint(equalTo: leadingAnchor),
            label.trailingAnchor.constraint(equalTo: trailingAnchor),
            label.centerYAnchor.constraint(equalTo: centerYAnchor),
            indicator.leadingAnchor.constraint(equalTo: leadingAnchor),
            indicator.trailingAnchor.constraint(equalTo: trailingAnchor),
            indicator.heightAnchor.constraint(equalToConstant: 2),
            indicator.bottomAnchor.constraint(equalTo: label.topAnchor, constant: -6),
        ])
        isAccessibilityElement = true
        accessibilityLabel = tab.label
        accessibilityIdentifier = "kbTab-\(tab.label)"
        setActive(false)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not from a nib") }

    func setActive(_ active: Bool) {
        indicator.isHidden = !active
        label.font = active ? PanelTheme.tabActiveFont : PanelTheme.tabFont
        label.textColor = active ? PanelTheme.ink : PanelTheme.meta
        accessibilityTraits = active ? [.button, .selected] : .button
    }
}

/// Bottom bar of the panel: five section tabs on the left, the ABC key
/// (system input-mode switcher) on the right, separated by a hairline rule.
final class PanelTabBar: UIView {
    var onSelect: ((PanelTab) -> Void)?
    /// Exposed so the controller can wire the system input-mode switcher.
    let abcKey = UIButton(type: .custom)

    private let topRule = UIView()
    private var items: [TabItemView] = []

    override init(frame: CGRect) {
        super.init(frame: frame)
        topRule.backgroundColor = PanelTheme.hairline
        topRule.translatesAutoresizingMaskIntoConstraints = false
        addSubview(topRule)

        let stack = UIStackView()
        stack.axis = .horizontal
        stack.spacing = 20
        stack.alignment = .fill
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)
        for tab in PanelTab.allCases {
            let item = TabItemView(tab: tab)
            item.addTarget(self, action: #selector(tapped(_:)), for: .touchUpInside)
            items.append(item)
            stack.addArrangedSubview(item)
        }

        abcKey.setTitle("ABC", for: .normal)
        abcKey.titleLabel?.font = PanelTheme.abcFont
        abcKey.setTitleColor(PanelTheme.preview, for: .normal)
        abcKey.accessibilityLabel = "Next keyboard"
        abcKey.accessibilityIdentifier = "kbGlobe"
        abcKey.translatesAutoresizingMaskIntoConstraints = false
        addSubview(abcKey)

        NSLayoutConstraint.activate([
            topRule.topAnchor.constraint(equalTo: topAnchor),
            topRule.leadingAnchor.constraint(equalTo: leadingAnchor),
            topRule.trailingAnchor.constraint(equalTo: trailingAnchor),
            topRule.heightAnchor.constraint(equalToConstant: 1),
            stack.leadingAnchor.constraint(equalTo: leadingAnchor, constant: PanelTheme.hInset),
            stack.topAnchor.constraint(equalTo: topAnchor),
            stack.bottomAnchor.constraint(equalTo: bottomAnchor),
            abcKey.trailingAnchor.constraint(
                equalTo: trailingAnchor, constant: -PanelTheme.hInset),
            abcKey.centerYAnchor.constraint(equalTo: centerYAnchor),
            abcKey.leadingAnchor.constraint(
                greaterThanOrEqualTo: stack.trailingAnchor, constant: 12),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not from a nib") }

    func configure(selected: PanelTab) {
        for item in items { item.setActive(item.tab == selected) }
    }

    @objc private func tapped(_ sender: UIControl) {
        guard let item = sender as? TabItemView else { return }
        onSelect?(item.tab)
    }
}

// MARK: - Rows

/// One list row. Three faces on the same anatomy: snippet (title + preview
/// line + ↵), locked vault entry (dot string + LOCKED, zero plaintext),
/// and folder (name + chevron).
final class SnippetRowCell: UITableViewCell {
    static let reuseId = "SnippetRowCell"

    private let title = UILabel()
    private let preview = UILabel()
    private let trailingMark = UILabel()
    private var titleTop: NSLayoutConstraint?
    private var titleCenter: NSLayoutConstraint?

    override init(style: UITableViewCell.CellStyle, reuseIdentifier: String?) {
        super.init(style: style, reuseIdentifier: reuseIdentifier)
        backgroundColor = .clear
        selectionStyle = .none

        preview.lineBreakMode = .byTruncatingTail
        // Body previews stay off the accessibility tree (design a11y rule).
        preview.isAccessibilityElement = false
        trailingMark.setContentCompressionResistancePriority(.required, for: .horizontal)
        trailingMark.isAccessibilityElement = false

        for view in [title, preview, trailingMark] {
            view.translatesAutoresizingMaskIntoConstraints = false
            contentView.addSubview(view)
        }
        let inset = PanelTheme.hInset
        titleTop = title.topAnchor.constraint(equalTo: contentView.topAnchor, constant: 12)
        titleCenter = title.centerYAnchor.constraint(equalTo: contentView.centerYAnchor)
        NSLayoutConstraint.activate([
            title.leadingAnchor.constraint(equalTo: contentView.leadingAnchor, constant: inset),
            title.trailingAnchor.constraint(
                lessThanOrEqualTo: trailingMark.leadingAnchor, constant: -12),
            preview.leadingAnchor.constraint(equalTo: title.leadingAnchor),
            preview.trailingAnchor.constraint(
                lessThanOrEqualTo: trailingMark.leadingAnchor, constant: -12),
            preview.topAnchor.constraint(equalTo: title.bottomAnchor, constant: 6),
            trailingMark.trailingAnchor.constraint(
                equalTo: contentView.trailingAnchor, constant: -inset),
            trailingMark.centerYAnchor.constraint(equalTo: contentView.centerYAnchor),
        ])
        isAccessibilityElement = true
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not from a nib") }

    /// Snippet face. `previewLine` is the first body line (nil for none);
    /// `highlight` underlines the first match of the query in the title.
    func configure(entry: SnapshotEntry, previewLine: String?, highlight query: String) {
        if entry.isSensitive {
            // Locked row: the snapshot holds only ciphertext for this entry,
            // so the row is a dot string and a LOCKED tag — no plaintext of
            // any kind, and no insert affordance.
            title.attributedText = NSAttributedString(
                string: "••••••••••",
                attributes: [
                    .font: PanelTheme.dotsFont,
                    .foregroundColor: PanelTheme.rowMeta,
                    .kern: PanelTheme.dotsKern,
                ])
            preview.isHidden = true
            setSingleLine(true)
            trailingMark.attributedText = NSAttributedString(
                string: "LOCKED",
                attributes: [
                    .font: PanelTheme.lockedTagFont,
                    .foregroundColor: PanelTheme.rowMeta,
                    .kern: PanelTheme.lockedTagKern,
                ])
            accessibilityLabel = "Locked, vault snippet"
            accessibilityTraits = .staticText
        } else {
            title.attributedText = Self.highlightedTitle(entry.title, query: query)
            preview.isHidden = previewLine == nil
            preview.text = previewLine
            preview.font = Self.monoPreviewTypes.contains(entry.snippetType)
                ? PanelTheme.rowPreviewMonoFont : PanelTheme.rowPreviewFont
            preview.textColor = PanelTheme.preview
            setSingleLine(previewLine == nil)
            trailingMark.attributedText = NSAttributedString(
                string: "↵",
                attributes: [
                    .font: PanelTheme.returnMarkFont,
                    .foregroundColor: PanelTheme.ghost,
                ])
            let word = TypeMark.word(for: TypeMark.mark(for: entry.snippetType))
            accessibilityLabel = "\(word), \(entry.title)"
            accessibilityTraits = .button
        }
        accessibilityIdentifier = "kbRow-\(entry.id)"
    }

    /// Folder face: name plus a ghost chevron; tapping drills in.
    func configure(folder: SnapshotFolder) {
        title.attributedText = NSAttributedString(
            string: folder.name,
            attributes: [
                .font: PanelTheme.rowTitleFont,
                .foregroundColor: PanelTheme.ink,
                .kern: PanelTheme.rowTitleKern,
            ])
        preview.isHidden = true
        setSingleLine(true)
        trailingMark.attributedText = NSAttributedString(
            string: "›",
            attributes: [
                .font: PanelTheme.returnMarkFont,
                .foregroundColor: PanelTheme.ghost,
            ])
        accessibilityLabel = "Folder, \(folder.name)"
        accessibilityTraits = .button
        accessibilityIdentifier = "kbFolder-\(folder.id)"
    }

    private func setSingleLine(_ single: Bool) {
        titleTop?.isActive = !single
        titleCenter?.isActive = single
    }

    /// Types whose preview line reads as machine content (mono face).
    private static let monoPreviewTypes: Set<String> = ["code", "command", "template"]

    private static func highlightedTitle(_ text: String, query: String) -> NSAttributedString {
        let attributed = NSMutableAttributedString(
            string: text,
            attributes: [
                .font: PanelTheme.rowTitleFont,
                .foregroundColor: PanelTheme.ink,
                .kern: PanelTheme.rowTitleKern,
            ])
        if !query.isEmpty,
            let range = text.range(of: query, options: [.caseInsensitive, .diacriticInsensitive])
        {
            attributed.addAttributes(
                [
                    .underlineStyle: NSUnderlineStyle.single.rawValue,
                    .underlineColor: PanelTheme.accent,
                ],
                range: NSRange(range, in: text))
        }
        return attributed
    }
}

// MARK: - Template fill

/// One template variable: caps mono label, borderless value field, and a
/// hairline underline that turns accent while the field is focused. The
/// field is an embedded UITextField — the established in-panel input path
/// (paste or a hardware keyboard; the extension IS the keyboard and cannot
/// summon another one).
final class TemplateFieldView: UIView {
    let name: String
    var onChange: (() -> Void)?

    private let caps = UILabel()
    private let field = UITextField()
    private let rule = UIView()

    init(name: String) {
        self.name = name
        super.init(frame: .zero)
        caps.attributedText = NSAttributedString(
            string: name.uppercased(),
            attributes: [
                .font: PanelTheme.capsFont,
                .foregroundColor: PanelTheme.rowMeta,
                .kern: PanelTheme.capsKern,
            ])
        field.font = PanelTheme.fieldValueFont
        field.textColor = PanelTheme.ink
        field.tintColor = PanelTheme.accent
        field.borderStyle = .none
        field.autocorrectionType = .no
        field.accessibilityLabel = name
        field.accessibilityIdentifier = "kbVar-\(name)"
        field.addTarget(self, action: #selector(changed), for: .editingChanged)
        field.addTarget(self, action: #selector(focusChanged), for: .editingDidBegin)
        field.addTarget(self, action: #selector(focusChanged), for: .editingDidEnd)
        rule.backgroundColor = PanelTheme.fieldRule
        for view in [caps, field, rule] {
            view.translatesAutoresizingMaskIntoConstraints = false
            addSubview(view)
        }
        NSLayoutConstraint.activate([
            caps.topAnchor.constraint(equalTo: topAnchor),
            caps.leadingAnchor.constraint(equalTo: leadingAnchor),
            caps.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor),
            field.topAnchor.constraint(equalTo: caps.bottomAnchor, constant: 8),
            field.leadingAnchor.constraint(equalTo: leadingAnchor),
            field.trailingAnchor.constraint(equalTo: trailingAnchor),
            field.heightAnchor.constraint(equalToConstant: 20),
            rule.topAnchor.constraint(equalTo: field.bottomAnchor, constant: 8),
            rule.leadingAnchor.constraint(equalTo: leadingAnchor),
            rule.trailingAnchor.constraint(equalTo: trailingAnchor),
            rule.heightAnchor.constraint(equalToConstant: 1),
            rule.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not from a nib") }

    var value: String { field.text ?? "" }

    @objc private func changed() { onChange?() }

    @objc private func focusChanged() {
        rule.backgroundColor = field.isFirstResponder ? PanelTheme.accent : PanelTheme.fieldRule
    }
}

/// Template fill face: variable fields two per row, a PREVIEW
/// section rendered locally on every change, and an ink Insert action with
/// an optional quiet Copy beside it.
final class TemplateFillView: UIView {
    var onChange: (() -> Void)?
    var onInsert: (() -> Void)?
    var onCopy: (() -> Void)?

    private let scroll = UIScrollView()
    private let content = UIStackView()
    private let fieldsStack = UIStackView()
    private let previewCaps = UILabel()
    private let previewLabel = UILabel()
    private let insertButton = UIButton(type: .custom)
    private let copyButton = UIButton(type: .custom)
    private var fields: [TemplateFieldView] = []

    override init(frame: CGRect) {
        super.init(frame: frame)
        content.axis = .vertical
        content.spacing = 20
        fieldsStack.axis = .vertical
        fieldsStack.spacing = 16
        previewCaps.attributedText = NSAttributedString(
            string: "PREVIEW",
            attributes: [
                .font: PanelTheme.capsFont,
                .foregroundColor: PanelTheme.rowMeta,
                .kern: PanelTheme.capsKern,
            ])
        previewLabel.numberOfLines = 0
        // The preview is derived content already reachable through the
        // fields; keep it off the accessibility tree like row previews.
        previewLabel.isAccessibilityElement = false
        let previewSection = UIStackView(arrangedSubviews: [previewCaps, previewLabel])
        previewSection.axis = .vertical
        previewSection.spacing = 10
        content.addArrangedSubview(fieldsStack)
        content.addArrangedSubview(previewSection)

        insertButton.backgroundColor = PanelTheme.actionBg
        insertButton.layer.cornerRadius = 11
        let insertTitle = NSMutableAttributedString(
            string: "Insert",
            attributes: [
                .font: PanelTheme.insertLabelFont,
                .foregroundColor: PanelTheme.actionText,
            ])
        insertTitle.append(
            NSAttributedString(
                string: "  ↵",
                attributes: [
                    .font: PanelTheme.insertReturnFont,
                    .foregroundColor: PanelTheme.actionText.withAlphaComponent(0.55),
                ]))
        insertButton.setAttributedTitle(insertTitle, for: .normal)
        insertButton.accessibilityLabel = "Insert"
        insertButton.accessibilityIdentifier = "kbTemplateInsert"
        insertButton.addTarget(self, action: #selector(insertTapped), for: .touchUpInside)

        copyButton.setAttributedTitle(
            NSAttributedString(
                string: "Copy",
                attributes: [
                    .font: PanelTheme.copyFont,
                    .foregroundColor: PanelTheme.secondary,
                ]),
            for: .normal)
        copyButton.accessibilityIdentifier = "kbTemplateCopy"
        copyButton.setContentHuggingPriority(.required, for: .horizontal)
        copyButton.addTarget(self, action: #selector(copyTapped), for: .touchUpInside)

        let actions = UIStackView(arrangedSubviews: [insertButton, copyButton])
        actions.axis = .horizontal
        actions.spacing = 18
        actions.alignment = .fill

        for view in [scroll, actions] {
            view.translatesAutoresizingMaskIntoConstraints = false
            addSubview(view)
        }
        content.translatesAutoresizingMaskIntoConstraints = false
        scroll.addSubview(content)

        let inset = PanelTheme.hInset
        NSLayoutConstraint.activate([
            scroll.topAnchor.constraint(equalTo: topAnchor),
            scroll.leadingAnchor.constraint(equalTo: leadingAnchor),
            scroll.trailingAnchor.constraint(equalTo: trailingAnchor),
            scroll.bottomAnchor.constraint(equalTo: actions.topAnchor, constant: -12),
            content.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor, constant: 18),
            content.leadingAnchor.constraint(
                equalTo: scroll.contentLayoutGuide.leadingAnchor, constant: inset),
            content.trailingAnchor.constraint(
                equalTo: scroll.contentLayoutGuide.trailingAnchor, constant: -inset),
            content.bottomAnchor.constraint(
                equalTo: scroll.contentLayoutGuide.bottomAnchor, constant: -12),
            content.widthAnchor.constraint(
                equalTo: scroll.frameLayoutGuide.widthAnchor, constant: -2 * inset),
            actions.leadingAnchor.constraint(equalTo: leadingAnchor, constant: inset),
            actions.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -inset),
            actions.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -16),
            actions.heightAnchor.constraint(equalToConstant: 44),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not from a nib") }

    /// Rebuilds the field grid (two per row) for a template's variables.
    func configure(variables: [String]) {
        fields = []
        fieldsStack.arrangedSubviews.forEach { $0.removeFromSuperview() }
        var index = 0
        while index < variables.count {
            let row = UIStackView()
            row.axis = .horizontal
            row.spacing = 18
            row.distribution = .fillEqually
            for name in variables[index..<min(index + 2, variables.count)] {
                let field = TemplateFieldView(name: name)
                field.onChange = { [weak self] in self?.onChange?() }
                fields.append(field)
                row.addArrangedSubview(field)
            }
            if row.arrangedSubviews.count == 1 {
                // Keep an odd last field at half width, like a full grid.
                row.addArrangedSubview(UIView())
            }
            fieldsStack.addArrangedSubview(row)
            index += 2
        }
        scroll.setContentOffset(.zero, animated: false)
    }

    /// Filled values only: missing keys keep their ‹name› placeholder in
    /// the preview and render empty on insert (FFI contract).
    var values: [String: String] {
        var filled: [String: String] = [:]
        for field in fields where !field.value.isEmpty {
            filled[field.name] = field.value
        }
        return filled
    }

    func setPreview(_ text: NSAttributedString) {
        previewLabel.attributedText = text
    }

    func setCopyVisible(_ visible: Bool) {
        copyButton.isHidden = !visible
    }

    @objc private func insertTapped() { onInsert?() }

    @objc private func copyTapped() { onCopy?() }
}

// MARK: - Vault locked figure

/// Centered locked state of the Vault tab: outlined circle
/// guarding the brand caret, a plain statement, and the reassurance line.
/// No unlock control — the keyboard process can never decrypt.
final class VaultLockedView: UIView {
    private let circle = UIView()
    private let caret = UIView()
    private let title = UILabel()
    private let sub = UILabel()

    override init(frame: CGRect) {
        super.init(frame: frame)
        circle.layer.borderWidth = 1
        circle.layer.cornerRadius = 20
        caret.backgroundColor = PanelTheme.accent
        title.attributedText = NSAttributedString(
            string: "Vault is locked",
            attributes: [
                .font: PanelTheme.vaultTitleFont,
                .foregroundColor: PanelTheme.ink,
                .kern: PanelTheme.vaultTitleKern,
            ])
        sub.text = "Nothing is inserted until you unlock"
        sub.font = PanelTheme.vaultSubFont
        sub.textColor = PanelTheme.preview
        for view in [circle, caret, title, sub] {
            view.translatesAutoresizingMaskIntoConstraints = false
        }
        addSubview(circle)
        circle.addSubview(caret)
        addSubview(title)
        addSubview(sub)
        NSLayoutConstraint.activate([
            circle.topAnchor.constraint(equalTo: topAnchor),
            circle.centerXAnchor.constraint(equalTo: centerXAnchor),
            circle.widthAnchor.constraint(equalToConstant: 40),
            circle.heightAnchor.constraint(equalToConstant: 40),
            caret.centerXAnchor.constraint(equalTo: circle.centerXAnchor),
            caret.centerYAnchor.constraint(equalTo: circle.centerYAnchor),
            caret.widthAnchor.constraint(equalToConstant: 2),
            caret.heightAnchor.constraint(equalToConstant: 14),
            title.topAnchor.constraint(equalTo: circle.bottomAnchor, constant: 22),
            title.centerXAnchor.constraint(equalTo: centerXAnchor),
            sub.topAnchor.constraint(equalTo: title.bottomAnchor, constant: 11),
            sub.centerXAnchor.constraint(equalTo: centerXAnchor),
            sub.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])
        applyDynamicLayerColors()
        isAccessibilityElement = true
        accessibilityLabel = "Vault is locked. Nothing is inserted until you unlock."
        accessibilityIdentifier = "kbVaultLocked"
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not from a nib") }

    override func traitCollectionDidChange(_ previous: UITraitCollection?) {
        super.traitCollectionDidChange(previous)
        if traitCollection.hasDifferentColorAppearance(comparedTo: previous) {
            applyDynamicLayerColors()
        }
    }

    private func applyDynamicLayerColors() {
        circle.layer.borderColor = PanelTheme.outline.resolvedColor(with: traitCollection).cgColor
    }
}

// MARK: - Inserted hint

/// Transient confirmation above the tab bar: a small caret bar
/// and "Inserted · keyboard stays open".
final class InsertedHintView: UIView {
    private let caret = UIView()
    private let label = UILabel()

    override init(frame: CGRect) {
        super.init(frame: frame)
        caret.backgroundColor = PanelTheme.accent
        label.text = "Inserted · keyboard stays open"
        label.font = PanelTheme.hintFont
        label.textColor = PanelTheme.meta
        for view in [caret, label] {
            view.translatesAutoresizingMaskIntoConstraints = false
            addSubview(view)
        }
        NSLayoutConstraint.activate([
            caret.leadingAnchor.constraint(equalTo: leadingAnchor),
            caret.centerYAnchor.constraint(equalTo: centerYAnchor),
            caret.widthAnchor.constraint(equalToConstant: 2),
            caret.heightAnchor.constraint(equalToConstant: 11),
            label.leadingAnchor.constraint(equalTo: caret.trailingAnchor, constant: 8),
            label.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor),
            label.topAnchor.constraint(equalTo: topAnchor),
            label.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])
        accessibilityIdentifier = "kbInsertedHint"
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not from a nib") }
}

// MARK: - Empty state

/// Text-built state (caret motif + two text lines); used for "no snapshot
/// yet", "no search matches", and empty tabs. EN main label, CN subtitle.
final class EmptyStateView: UIView {
    private let caret = UIView()
    private let main = UILabel()
    private let sub = UILabel()

    override init(frame: CGRect) {
        super.init(frame: frame)
        caret.backgroundColor = PanelTheme.accent
        main.font = UIFont.systemFont(ofSize: 14.5)
        main.textColor = PanelTheme.secondary
        main.textAlignment = .center
        main.numberOfLines = 0
        sub.font = PanelTheme.hintFont
        sub.textColor = PanelTheme.meta
        sub.textAlignment = .center
        for view in [caret, main, sub] {
            view.translatesAutoresizingMaskIntoConstraints = false
            addSubview(view)
        }
        NSLayoutConstraint.activate([
            caret.centerXAnchor.constraint(equalTo: centerXAnchor),
            caret.topAnchor.constraint(equalTo: topAnchor, constant: 44),
            caret.widthAnchor.constraint(equalToConstant: 2),
            caret.heightAnchor.constraint(equalToConstant: 22),
            main.topAnchor.constraint(equalTo: caret.bottomAnchor, constant: 14),
            main.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 24),
            main.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -24),
            sub.topAnchor.constraint(equalTo: main.bottomAnchor, constant: 6),
            sub.centerXAnchor.constraint(equalTo: centerXAnchor),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not from a nib") }

    func show(main mainText: String, sub subText: String) {
        main.text = mainText
        sub.text = subText
        // CJK line-height rule (+0.15 over EN) matters only when the CN
        // line wraps; the subtitle is a single line at this width.
    }
}
