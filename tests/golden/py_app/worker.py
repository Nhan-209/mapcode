class Worker:
    def __init__(self, name: str):
        self.name = name

    def run(self):
        return f"worker {self.name} running"

    def close(self):
        pass

def run():
    return "standalone python run"
