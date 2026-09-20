# tests/fixtures/sample_workspace/python_app/app.py

from fastapi import FastAPI
from .models import Product

app = FastAPI(title="Sample Python API")

@app.get("/items")
def list_items():
    p = Product(id=1, name="Widget", price=19.99)
    return [{"id": p.id, "name": p.name, "price": p.format_price()}]

@app.post("/items")
def create_item(name: str, price: float):
    return {"status": "created", "name": name, "price": price}

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8000)
