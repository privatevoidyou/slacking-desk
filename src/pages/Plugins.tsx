import { useState } from "react";
import { PackageOpen, Search, Upload } from "lucide-react";
import { useT } from "../i18n";

type Tab = "installed" | "market" | "updates";

export default function Plugins() {
  const t = useT();
  const [tab, setTab] = useState<Tab>("installed");
  const tabs: Tab[] = ["installed", "market", "updates"];
  return (
    <>
      <header className="hd">
        <h1>{t("plugins.title")}</h1>
        <div className="tools">
          <div className="search">
            <Search size={17} />
            {t("plugins.search")}
          </div>
          <button className="btn" disabled>
            <Upload size={17} />
            {t("plugins.fromFile")}
          </button>
        </div>
      </header>
      <div className="tabs">
        {tabs.map((id) => (
          <button key={id} className={tab === id ? "tab on" : "tab"} onClick={() => setTab(id)}>
            {t(`plugins.${id}`)}
          </button>
        ))}
      </div>
      <div className="empty">
        <div className="empty-ico">
          <PackageOpen size={28} />
        </div>
        <h2>{tab === "market" ? t("plugins.marketSoon") : t("plugins.empty")}</h2>
      </div>
    </>
  );
}
