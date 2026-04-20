"""Room navigation with history stack."""

from typing import Optional

class RoomNavigator:
    def __init__(self, max_history: int = 100):
        self._current: str = ""
        self._history: list[str] = []
        self._forward: list[str] = []
        self._breadcrumbs: list[str] = []
        self.max_history = max_history

    def go(self, room: str):
        if self._current:
            self._history.append(self._current)
            if len(self._history) > self.max_history:
                self._history.pop(0)
        self._forward.clear()
        self._current = room
        if room not in self._breadcrumbs:
            self._breadcrumbs.append(room)

    def back(self) -> Optional[str]:
        if not self._history:
            return None
        self._forward.append(self._current)
        self._current = self._history.pop()
        return self._current

    def forward(self) -> Optional[str]:
        if not self._forward:
            return None
        self._history.append(self._current)
        self._current = self._forward.pop()
        return self._current

    def jump(self, room: str):
        """Jump without pushing current to back stack."""
        self._forward.clear()
        self._current = room
        if room not in self._breadcrumbs:
            self._breadcrumbs.append(room)

    @property
    def current(self) -> str:
        return self._current

    @property
    def breadcrumbs(self) -> list[str]:
        return list(self._breadcrumbs)

    @property
    def can_go_back(self) -> bool:
        return len(self._history) > 0

    @property
    def can_go_forward(self) -> bool:
        return len(self._forward) > 0

    @property
    def history_depth(self) -> int:
        return len(self._history)

    def clear_history(self):
        self._history.clear()
        self._forward.clear()
        self._breadcrumbs.clear()

    @property
    def stats(self) -> dict:
        return {"current": self._current, "breadcrumbs": self._breadcrumbs,
                "history_depth": len(self._history), "forward_depth": len(self._forward)}
