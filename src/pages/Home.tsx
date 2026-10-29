import { Clock, Pin, Plus, Search, Sparkles } from "lucide-react";
import { useT } from "../i18n";

function greetingKey() {
  const h = new Date().getHours();
  if (h < 5) return "greet.night" as const;
  if (h < 11) return "greet.morning" as const;
  if (h < 13) return "greet.noon" as const;
  if (h < 18) return "greet.afternoon" as const;
  return "greet.evening" as const;
}

export default function Home({ onGoPlugins }: { onGoPlugins: () => void }) {
  const t = useT();
  return (
    <>
      <header className="hd">
        <div>
          <h1>{t(greetingKey())}</h1>
          <p className="sub">{t("home.sub")}</p>
        </div>
        <div className="tools">
          <div className="search">
            <Search size={17} />
            {t("home.search")}
          </div>
          <div className="seg">
            <b className="on">
              <Clock size={15} />
              {t("home.recent")}
            </b>
            <b>
              <Pin size={15} />
              {t("home.fixed")}
            </b>
          </div>
          <button className="btn pri" disabled>
            <Plus size={17} />
            {t("home.add")}
          </button>
        </div>
      </header>
      <div className="empty">
        <div className="empty-ico">
          <Sparkles size={28} />
        </div>
        <h2>{t("home.emptyTitle")}</h2>
        <p>{t("home.emptyDesc")}</p>
        <button className="btn" onClick={onGoPlugins}>
          {t("home.emptyCta")}
        </button>
      </div>
    </>
  );
}
