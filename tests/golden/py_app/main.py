from worker import Worker

def execute():
    w = Worker("data-sync")
    msg = w.run()
    w.close()

if __name__ == "__main__":
    execute()
