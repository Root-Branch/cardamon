import socket
import time

def stress_test(host, port, duration):
    start_time = time.time()
    while time.time() - start_time < duration:
        try:
            client_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            client_socket.connect((host, port))
            client_socket.send('Stress test data'.encode('utf-8'))
            response = client_socket.recv(1024).decode('utf-8')
            print(f"Received response: {response}")
            client_socket.close()
        except ConnectionRefusedError:
            print("Connection refused. Retrying...")
        time.sleep(0.1)

def main():
    host = 'localhost'  
    port = 8000
    duration = 15 # Stress test duration in seconds

    stress_test(host, port, duration)

if __name__ == '__main__':
    main()
