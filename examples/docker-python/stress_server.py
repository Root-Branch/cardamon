import multiprocessing
import socket
import time

def cpu_stress():
    while True:
        for _ in range(10000000):
            pass

def memory_stress():
    allocations = []
    while True:
        allocations.append(' ' * 10485760)  # Allocate 10 MB
        time.sleep(0.1)

def handle_client(conn):
    while True:
        data = conn.recv(1024).decode('utf-8')
        if not data:
            break
        conn.send(data.encode('utf-8'))
    conn.close()

def main():
    num_cpu_stressors = multiprocessing.cpu_count()
    num_mem_stressors = 2

    for _ in range(num_cpu_stressors):
        multiprocessing.Process(target=cpu_stress).start()

    for _ in range(num_mem_stressors):
        multiprocessing.Process(target=memory_stress).start()

    server_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server_socket.bind(('0.0.0.0', 8000))
    server_socket.listen(1)

    print("Stress server is running...")

    while True:
        conn, _ = server_socket.accept()
        multiprocessing.Process(target=handle_client, args=(conn,)).start()

if __name__ == '__main__':
    main()
