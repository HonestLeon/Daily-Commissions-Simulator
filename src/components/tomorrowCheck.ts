import { Plugin, PluginKey } from "@milkdown/prose/state";
import { Decoration, DecorationSet, EditorView } from "@milkdown/prose/view";
import type { Node } from "@milkdown/prose/model";
import { $prose } from "@milkdown/utils";
import { listItemSchema, blockquoteSchema } from "@milkdown/preset-commonmark";

const tomorrowKey = new PluginKey("tomorrow-item");

/** 当前已挂载的编辑器视图（本应用同一时刻仅一个编辑器） */
const mountedViews = new Set<EditorView>();

const CHECK_UNCHECKED_SVG =
  '<svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2"/></svg>';
const CHECK_CHECKED_SVG =
  '<svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2"/><path d="M9 12l2 2 4-4"/></svg>';

/** 块内第一个文本块（段落）内容末尾的相对位置，用于放置小部件 */
function textEndPos(node: Node): number | null {
  let end: number | null = null;
  node.forEach((child, offset) => {
    if (end == null && child.isTextblock) {
      end = offset + child.nodeSize - 1;
    }
  });
  return end;
}

/** 收集块内所有文本块的子节点（行内内容） */
function collectInline(node: Node): Node[] {
  const inline: Node[] = [];
  node.forEach((child) => {
    if (child.isTextblock) {
      child.forEach((n) => inline.push(n));
    }
  });
  return inline;
}

/** 从 widget 所在 DOM 向上查找编辑器视图 */
function findViewFromWidget(el: HTMLElement): EditorView | null {
  const proseMirror = el.closest(".ProseMirror");
  if (!proseMirror) return null;
  for (const v of mountedViews) {
    if (v.dom === proseMirror) return v;
  }
  return null;
}

/**
 * 为每条任务项（- [ ]）和引用块（> 明日委托）在末尾追加一个勾选框：
 * - 任务项：勾选后转换为引用块（标记为明日委托）
 * - 引用块：勾选后转换回普通任务
 */
export const tomorrowCheckProse = $prose((ctx) => {
  const listItemType = listItemSchema.type(ctx);
  const blockquoteType = blockquoteSchema.type(ctx);

  const makeButton = (isTask: boolean, absPos: number): HTMLElement => {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "tomorrow-check";
    btn.setAttribute("contenteditable", "false");
    btn.title = isTask ? "标记为明日委托（顺延到明天）" : "取消明日委托（变回普通任务）";
    btn.innerHTML = isTask ? CHECK_UNCHECKED_SVG : CHECK_CHECKED_SVG;

    const onAction = (e: Event) => {
      e.preventDefault();
      e.stopPropagation();
      const view = findViewFromWidget(btn);
      if (!view) return;

      const { doc, schema } = view.state;
      const node = doc.nodeAt(absPos);
      if (!node) return;

      if (isTask && node.type === listItemType && node.attrs.checked != null) {
        // 任务 -> 引用块（明日委托）
        const para = schema.nodes.paragraph.create(null, collectInline(node));
        const quote = schema.nodes.blockquote.create(null, para);
        let from = absPos;
        let to = absPos + node.nodeSize;
        const $pos = doc.resolve(absPos);
        const parent = $pos.parent;
        // 若该任务项是所在列表的唯一子项，则整体替换列表，避免产生空列表
        if (
          (parent.type.name === "bullet_list" || parent.type.name === "ordered_list") &&
          parent.childCount === 1
        ) {
          const depth = $pos.depth;
          from = $pos.before(depth - 1);
          to = from + $pos.node(depth - 1).nodeSize;
        }
        view.dispatch(view.state.tr.replaceWith(from, to, quote));
      } else if (!isTask && node.type === blockquoteType) {
        // 引用块 -> 任务
        const para = schema.nodes.paragraph.create(null, collectInline(node));
        const li = schema.nodes.list_item.create(
          { label: "•", listType: "bullet", spread: true, checked: false },
          para
        );
        const list = schema.nodes.bullet_list.create(null, li);
        view.dispatch(view.state.tr.replaceWith(absPos, absPos + node.nodeSize, list));
      }
    };

    // 阻止 mousedown 使光标移入按钮/编辑器失焦
    btn.addEventListener("mousedown", (e) => {
      e.preventDefault();
      e.stopPropagation();
    });
    btn.addEventListener("click", onAction);
    return btn;
  };

  return new Plugin({
    key: tomorrowKey,
    view(view) {
      mountedViews.add(view);
      return {
        update() {
          // 不需要额外处理
        },
        destroy() {
          mountedViews.delete(view);
        },
      };
    },
    props: {
      decorations(state) {
        const decos: Decoration[] = [];
        state.doc.descendants((node, pos) => {
          if (node.type === listItemType && node.attrs.checked != null) {
            const end = textEndPos(node);
            if (end != null) {
              decos.push(Decoration.widget(pos + end, () => makeButton(true, pos), { side: -1 }));
            }
          } else if (node.type === blockquoteType) {
            const end = textEndPos(node);
            if (end != null) {
              decos.push(Decoration.widget(pos + end, () => makeButton(false, pos), { side: -1 }));
            }
          }
        });
        return DecorationSet.create(state.doc, decos);
      },
    },
  });
});