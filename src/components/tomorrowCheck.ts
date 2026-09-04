import { Plugin, PluginKey } from "@milkdown/prose/state";
import { Decoration, DecorationSet, EditorView } from "@milkdown/prose/view";
import type { Node } from "@milkdown/prose/model";
import { $prose } from "@milkdown/utils";
import {
  listItemSchema,
  blockquoteSchema,
  headingSchema,
} from "@milkdown/preset-commonmark";

const tomorrowKey = new PluginKey("tomorrow-item");

/** 「明日委托」勾选框仅对这两个模块内的项显示（与后端结算/继承的模块划分一致） */
const TOMORROW_ALLOWED_SECTIONS = new Set(["学习/工作", "生活/其他"]);

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
      // 父节点内容区从 +1 开始；段落开标记在 1+offset，段内文本末尾 = 开标记 + nodeSize - 1
      // 即相对偏移 = offset + child.nodeSize
      end = offset + child.nodeSize;
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
  const headingType = headingSchema.type(ctx);

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
        // 任务 -> 引用块（明日委托）：显式重建父列表，确定性避免残留空列表
        const $pos = doc.resolve(absPos);
        const list = $pos.parent; // bullet_list / ordered_list
        const quote = schema.nodes.blockquote.create(
          null,
          schema.nodes.paragraph.create(null, collectInline(node))
        );
        // 用片段序列替换整个父列表：目标项换为引用块，其余项按连续段重新分组
        const parts: Node[] = [];
        let group: Node[] = [];
        const flushGroup = () => {
          if (group.length > 0) {
            parts.push(list.type.create(list.attrs, group));
            group = [];
          }
        };
        list.forEach((child) => {
          if (child === node) {
            flushGroup();
            parts.push(quote);
          } else {
            group.push(child);
          }
        });
        flushGroup();
        view.dispatch(
          view.state.tr.replaceWith($pos.before(), $pos.before() + list.nodeSize, parts)
        );
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
        // 光标所在的“项”（任务项/引用块）不显示勾选框；
        // 注意：只跳过光标所在的目标节点本身，不跳过其祖先（如整个列表），
        // 否则同列表的其他项也会被一并隐藏。
        const selFrom = state.selection.from;
        // 追踪当前所在模块：命中 level-4 标题（#### 必做 等）时更新。
        // 与后端 parse_md_sections 的模块划分保持一致。
        let inAllowedSection = false;
        state.doc.descendants((node, pos) => {
          if (node.type === headingType && node.attrs.level === 4) {
            inAllowedSection = TOMORROW_ALLOWED_SECTIONS.has(
              node.textContent.trim()
            );
            return true;
          }
          const isTask = node.type === listItemType && node.attrs.checked != null;
          const isQuote = node.type === blockquoteType;
          if (!isTask && !isQuote) return; // 非目标节点，继续向下遍历
          if (!inAllowedSection) return false; // 必做 / 随手小记 / 无模块内容不显示勾选框
          if (selFrom > pos && selFrom < pos + node.nodeSize) {
            return false; // 光标在这一项内（输入中）→ 不显示
          }
          const end = textEndPos(node);
          if (end != null) {
            decos.push(
              Decoration.widget(pos + end, () => makeButton(isTask, pos), { side: -1 })
            );
          }
          return false; // 目标节点内部无需再遍历
        });
        return DecorationSet.create(state.doc, decos);
      },
    },
  });
});