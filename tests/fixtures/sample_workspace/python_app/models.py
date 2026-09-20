# tests/fixtures/sample_workspace/python_app/models.py

from dataclasses import dataclass
from typing import Optional

@dataclass
class BaseEntity:
    id: int

    def get_id(self) -> int:
        return self.id


@dataclass
class Product(BaseEntity):
    name: str
    price: float
    description: Optional[str] = None

    def format_price(self) -> str:
        return f"${self.price:.2f}"
