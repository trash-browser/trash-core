import { useEffect } from "react";
import {
  HashRouter,
  Navigate,
  Route,
  Routes,
  useLocation,
} from "react-router-dom";
import { AnimatePresence } from "framer-motion";
import { SettingsListPage, SettingsShell } from "./pages/SettingsLayout";
import { SettingsSubpage } from "./pages/SettingsSubpage";
import { NewTabPage } from "./pages/NewTabPage";
import { DownloadManagerPage } from "./pages/DownloadManagerPage";
import { AdblockManagerPage } from "./pages/AdblockManagerPage";
import { OverlayPage } from "./overlay/OverlayPage";
import { PageTransition } from "./motion";
import { DEFAULT_NAV } from "./data/settings";

function useOverlayDocumentClass() {
  const location = useLocation();
  useEffect(() => {
    const isOverlay = location.pathname.startsWith("/overlay");
    document.documentElement.classList.toggle("overlay-mode", isOverlay);
    document.body.classList.toggle("overlay-mode", isOverlay);
    return () => {
      document.documentElement.classList.remove("overlay-mode");
      document.body.classList.remove("overlay-mode");
    };
  }, [location.pathname]);
}

function AnimatedRoutes() {
  useOverlayDocumentClass();
  const location = useLocation();

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
    <AnimatePresence mode="wait">
      <Routes location={location} key={location.pathname.split("/").slice(0, 3).join("/")}>
        <Route
          index
          element={
            <PageTransition>
              <Navigate to="/new-tab" replace />
            </PageTransition>
          }
        />
        <Route
          path="new-tab"
          element={
            <PageTransition>
              <NewTabPage />
            </PageTransition>
          }
        />
        <Route
          path="download-manager"
          element={
            <PageTransition>
              <DownloadManagerPage />
            </PageTransition>
          }
        />
        <Route
          path="adblock"
          element={
            <PageTransition>
              <AdblockManagerPage />
            </PageTransition>
          }
        />
        <Route
          path="settings"
          element={<Navigate to={`/settings/${DEFAULT_NAV}`} replace />}
        />
        <Route path="settings/:page" element={<SettingsShell />}>
          <Route
            index
            element={
              <PageTransition>
                <SettingsListPage />
              </PageTransition>
            }
          />
          <Route
            path=":sub"
            element={
              <PageTransition>
                <SettingsSubpage />
              </PageTransition>
            }
          />
        </Route>
        <Route path="overlay/:kind" element={<OverlayPage />} />
        <Route path="*" element={<Navigate to="/new-tab" replace />} />
      </Routes>
    </AnimatePresence>
    </div>
  );
}

export default function App() {
  return (
    <HashRouter>
      <AnimatedRoutes />
    </HashRouter>
  );
}
