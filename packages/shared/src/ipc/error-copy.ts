// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Single-language copy for backend errors. The Rust side deliberately
 * sends static English sentences (log red line: no payload in errors); this
 * table maps each known sentence to its Chinese reading so zh mode never
 * shows raw English. Display sites render `tr(...ipcErrorCopy(caught))`.
 *
 * An unknown sentence keeps the backend's English and falls back to a
 * generic per-code Chinese line — honest about the outcome without
 * pretending to know the specifics. The only dynamic backend messages
 * ("fill in the required field: …", provider status codes) take this path
 * by design.
 */
import { IpcError, type IpcErrorCode } from './error';

/** An (en, zh) pair in `useTr` argument order. */
export type ErrorCopy = readonly [en: string, zh: string];

const KNOWN: Readonly<Record<string, string>> = {
  // --- validation ----------------------------------------------------------
  'a device cannot revoke itself': '设备不能撤销自己',
  'a triggered snippet has an invalid trigger': '有片段的触发词无效',
  'could not parse the Espanso file': '无法解析这个 Espanso 文件',
  'draft body must not be blank': '正文不能为空',
  "enter a master password for this device's vault": '请为本机保险库设置主密码',
  'espanso is not available': 'Espanso 不可用',
  'folder view requires folderId': '文件夹视图缺少 folderId',
  'folderId only applies to the folder view': 'folderId 仅用于文件夹视图',
  'limit must be between 1 and 500': '分页大小须在 1–500 之间',
  'no snippets selected': '未选择任何片段',
  'not an Espanso match file': '这不是 Espanso match 文件',
  'secret fields are filled after unlocking the vault': '密钥字段需解锁保险库后填写',
  'server address must start with https://': '服务器地址必须以 https:// 开头',
  'template could not be parsed': '模板无法解析',
  'template references an undefined field': '模板引用了未定义的字段',
  'template text must not be blank': '模板内容不能为空',
  'that is not a Typvia pairing code': '这不是 Typvia 配对码',
  'that recovery code is not complete': '恢复码不完整',
  'there is no input text to run on': '没有可运行的输入文本',
  'too many snippets in one batch': '一次选择的片段过多',
  'unknown import format': '未知的导入格式',
  'unknown injection method': '未知的注入方式',
  'unknown input source': '未知的输入来源',
  'unknown library view': '未知的库视图',
  'unknown output mode': '未知的输出方式',
  'unknown permission scope': '未知的权限范围',
  'unknown provider kind': '未知的 Provider 类型',
  'unknown rule type': '未知的规则类型',
  'unknown snippet type': '未知的片段类型',
  'unknown template field type': '未知的模板字段类型',
  'unknown trigger mode': '未知的触发方式',
  'sensitive snippets cannot be injected yet': '敏感片段暂不支持注入',
  'the backup passphrase must not be empty': '备份口令不能为空',
  'restore needs an empty library — this one already has content':
    '恢复仅可在空库中进行——当前库已有内容',
  'app rules are not supported on this platform': '此平台不支持应用规则',
  'per-app expansion control is not available yet': '按应用控制展开暂不可用',
  "the input does not come from this action's configured source": '输入与该动作配置的来源不符',
  'the input is too long for an AI action — select a smaller part': '输入过长——请选择更小的部分',
  'this action has no provider configured — pick one in its settings':
    '该动作未配置 Provider——请在其设置中选择',
  'the AI returned an empty answer — try again': 'AI 返回了空答案——请重试',
  // --- conflict ------------------------------------------------------------
  'a record could not be verified': '有一条记录未能通过验证',
  'already the current version': '已经是当前版本',
  'biometric unlock is not set up': '尚未设置生物识别解锁',
  'confirm the pairing code first': '请先确认配对码',
  'no pairing is in progress': '没有正在进行的配对',
  'snippet is already sensitive': '该片段已是敏感片段',
  'snippet is not a conflict copy': '该片段不是冲突副本',
  'snippet is not sensitive': '该片段不是敏感片段',
  'snippet is sensitive': '该片段是敏感片段',
  'sync is not set up on this device': '本机尚未设置同步',
  'that code belongs to a different account': '这个码属于另一个账户',
  'the AI answer could not be used — try again or rephrase the draft':
    'AI 的回答无法使用——请重试或改写草稿',
  'the pairing answer could not be opened': '配对应答无法解封',
  'the server refused this request': '服务器拒绝了这次请求',
  "the server's answer was out of order; nothing was applied": '服务器返回顺序异常——未应用任何内容',
  'this account has no vault to recover': '该账户没有可恢复的保险库',
  "this account's trust root could not be verified": '该账户的信任根未能通过验证',
  'this action no longer exists': '该动作已不存在',
  'this AI provider is not configured': '该 AI Provider 未配置',
  'this device already has its own vault': '本机已有自己的保险库',
  'this device has no secure key storage': '本机没有可用的安全密钥存储',
  'this platform cannot join an account': '此平台无法加入账户',
  'this rule already exists': '该规则已存在',
  'this server speaks a different protocol version': '服务器协议版本不一致',
  'trigger already in use': '触发词已被占用',
  'this WebDAV endpoint cannot keep files consistent':
    '这个 WebDAV 端点无法保证文件一致性(不支持条件请求)',
  'this storage already holds an account — join it by pairing':
    '这个存储上已有账户——请通过配对加入',
  'the WebDAV endpoint refused these credentials': 'WebDAV 端点拒绝了这组凭据',
  'vault is already set up': '保险库已设置',
  'vault is not set up': '保险库尚未设置',
  'sensitive snippets are edited through the vault flow': '敏感片段需在保险库中编辑',
  'this provider requires an API key': '该 Provider 需要 API Key',
  'request cancelled': '请求已取消',
  'this looks like it contains a secret, so nothing was sent — remove it, or save it as a sensitive snippet instead':
    '内容疑似包含密钥,因此什么也没有发送——请移除它,或改存为敏感片段',
  'sensitive snippets never leave this device, so nothing was sent':
    '敏感片段永远不会离开本机——因此什么也没有发送',
  'the provider rejected the API key': 'Provider 拒绝了这个 API Key',
  'the provider does not know the configured model': 'Provider 不认识配置的模型',
  // --- permission_denied ---------------------------------------------------
  'that recovery code did not open this account': '恢复码未能打开该账户',
  'the pairing answer could not be verified': '配对应答未能通过验证',
  'this device was revoked from the account': '本机已被从账户中撤销',
  'this device was signed out': '本机会话已失效',
  'too many attempts, try again later': '尝试次数过多,请稍后再试',
  'unlock failed': '解锁失败',
  'unlock the vault first': '请先解锁保险库',
  // --- unavailable ---------------------------------------------------------
  'the server could not be reached': '无法连接服务器',
  'waiting before the next connection attempt': '正在等待下一次连接尝试',
  'the provider is rate limiting requests': 'Provider 正在限流',
  'the provider did not answer in time': 'Provider 未在时限内响应',
  'the provider could not be reached': '无法连接 Provider',
  // --- not_found / system --------------------------------------------------
  'not found': '未找到',
  'internal storage error': '内部存储错误——你的数据未受影响',
  'unexpected IPC failure': '通信异常——请重试',
};

/** Per-code generic zh line for sentences not in the table. */
const CODE_FALLBACK_ZH: Readonly<Record<IpcErrorCode, string>> = {
  validation: '输入内容无法使用,请检查后重试',
  conflict: '操作与当前状态冲突',
  not_found: '未找到',
  permission_denied: '没有权限执行此操作',
  rule_blocked: '应用规则阻止了此操作',
  unavailable: '暂时无法连接',
  system: '内部错误——你的数据未受影响',
};

/**
 * The (en, zh) copy for a caught error. Known backend sentences translate
 * exactly; unknown ones keep the English and use the per-code generic zh;
 * non-IPC failures collapse to the system line.
 */
export function ipcErrorCopy(caught: unknown): ErrorCopy {
  if (caught instanceof IpcError) {
    const zh = KNOWN[caught.message];
    return [caught.message, zh ?? CODE_FALLBACK_ZH[caught.code]];
  }
  if (caught instanceof Error && caught.message !== '') {
    return [caught.message, CODE_FALLBACK_ZH.system];
  }
  return ['something went wrong', CODE_FALLBACK_ZH.system];
}
