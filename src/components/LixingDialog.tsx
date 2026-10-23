import type { WeeklyResult } from "../types";

interface Props {
  result: WeeklyResult | null;
  onClose: () => void;
}

export default function LixingDialog({ result, onClose }: Props) {
  if (!result) return null;

  return (
    <>
      <div className="dialog-overlay" onClick={onClose} />
      <div className="dialog-panel" style={{ width: 340 }}>
        <div className="dialog-header">
          <span>砺行修远</span>
          <button className="dialog-close" onClick={onClose}>
            ✕
          </button>
        </div>
        <div className="dialog-body">
          <p style={{ fontSize: 14, color: "#888", marginTop: 0 }}>
            本周（周一至今日）委托完成进度：
          </p>

          <div style={{ marginBottom: 12 }}>
            {result.days.length > 0 ? (
              result.days.map((d) => (
                <div
                  key={d.day}
                  style={{
                    display: "flex",
                    justifyContent: "space-between",
                    padding: "4px 8px",
                    fontSize: 13,
                  }}
                >
                  <span>{d.day}</span>
                  <span
                    style={{
                      color: d.triple ? "#27ae60" : "#aaa",
                    }}
                  >
                    {d.triple ? "✅ 三全完成" : "未完成"}
                  </span>
                </div>
              ))
            ) : (
              <p style={{ fontSize: 13, color: "#aaa", textAlign: "center" }}>本周还没有委托记录</p>
            )}
          </div>

          <div
            style={{
              padding: "10px 12px",
              borderRadius: 8,
              background: "rgba(0,0,0,0.04)",
              fontSize: 14,
            }}
          >
            {result.eligible ? (
              <>
                <div>
                  三全完成天数：<strong>{result.count}</strong> 天（≥5 达标）
                </div>
                <div style={{ marginTop: 6 }}>
                  {result.reward > 0 ? (
                    <>
                      🪨 获得奖励：{" "}
                      <strong style={{ color: "#e67e22" }}>{result.reward} 原石</strong>
                    </>
                  ) : (
                    <>🪨 本周奖励已领取（只结算一次）</>
                  )}
                </div>
                <div style={{ marginTop: 6, color: "#888" }}>
                  砺行连续周数：{result.streak} 周
                  {result.streak === 0 && "（已满 7 周清零，或中断清零）"}
                </div>
              </>
            ) : (
              <>
                <div>
                  三全完成天数：<strong>{result.count}</strong> 天（未达 5 天）
                </div>
                <div style={{ marginTop: 6, color: "#c0392b" }}>
                  本周还未达标，还需 {Math.max(0, 5 - result.count)} 天三全完成
                </div>
                <div style={{ marginTop: 6, color: "#888" }}>砺行连续周数：{result.streak} 周</div>
              </>
            )}
          </div>

          <p
            style={{
              fontSize: 12,
              color: "var(--text-muted)",
              marginTop: 12,
              lineHeight: 1.6,
            }}
          >
            提示：任一模块只有空委托（未填写实际任务）时，不计入该模块完成，也不会计入"三全完成"天数。
          </p>
        </div>
      </div>
    </>
  );
}
