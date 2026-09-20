# tests/fixtures/sample_workspace/python_app/worker.py

from celery import Celery

celery_app = Celery('tasks', broker='pyamqp://guest@localhost//')

@celery_app.task
def process_order(order_id: int):
    print(f"Processing background order {order_id}")
    return {"status": "completed", "order_id": order_id}

@celery_app.task
def send_email_notification(recipient: str, message: str):
    print(f"Sending email to {recipient}")
    return True
