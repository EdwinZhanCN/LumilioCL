import { useEffect, useState } from "react";

/** False on the server and on the first client render, so anything that
 * depends on the browser (screen width, reduced motion) hydrates as the
 * server rendered it and changes only after mount. */
export function useMounted(): boolean {
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);
  return mounted;
}
