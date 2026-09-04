import { Plugin, PluginKey } from "@milkdown/prose/state";
import { $prose } from "@milkdown/utils";
import { listItemSchema } from "@milkdown/preset-commonmark";

const enterTaskResetKey = new PluginKey("enter-task-reset");

/**
 * 回车分裂列表项时，prosemirror-transform 的 split() 会对空 typesAfter 层级执行
 * node.copy()，从而把源 list_item 的 checked 一并克隆：从已完成任务 - [x] 回车，
 * 新行会继承完成状态。这里兜底：任何“内容为空却已勾选”的任务项一律重置为未完成，
 * 保证回车自动补出的待办项都是 - [ ]。
 */
export const enterTaskResetProse = $prose((ctx) => {
  const listItemType = listItemSchema.type(ctx);

  return new Plugin<unknown>({
    key: enterTaskResetKey,
    appendTransaction(_transactions, _oldState, newState) {
      let tr: ReturnType<typeof newState.tr.setNodeMarkup> | null = null;
      newState.doc.descendants((node, pos) => {
        if (node.type !== listItemType) return;
        if (node.attrs.checked !== true) return;
        // 判定任务项是否为空（无文本、无嵌套内容）
        let hasContent = false;
        node.forEach((child) => {
          if (child.isTextblock) {
            if (child.textContent.trim().length > 0) hasContent = true;
          } else {
            hasContent = true;
          }
        });
        if (hasContent) return;
        tr = (tr ?? newState.tr).setNodeMarkup(pos, undefined, {
          ...node.attrs,
          checked: false,
        });
      });
      return tr;
    },
  });
});