#include <hal/HAL.h>
#include <hal/CAN.h>

#include <unistd.h>
#include <sys/types.h> 
#include <sys/socket.h>
#include <netinet/in.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <iostream>
#include <thread>

int main() {
  using namespace std::chrono_literals;

  if (HAL_Initialize(500, 0) == 0) {
    std::cout << "Failed to Initialise the HAL" << std::endl;
    return 1;
  }

  int port = 8006;
  auto sockfd = socket(AF_INET, SOCK_STREAM, 0);
  if (sockfd < 0) {
    std::cout << "Error opening socket" << std::endl;
    return 1;
  }

  struct sockaddr_in serv_addr, client_addr;
  bzero((char *) &serv_addr, sizeof(serv_addr));

  serv_addr.sin_family = AF_INET;
  serv_addr.sin_addr.s_addr = INADDR_ANY;
  serv_addr.sin_port = htons(port);
  int reuse = 1;
  setsockopt(sockfd, SOL_SOCKET, SO_REUSEADDR, &reuse, sizeof(int));
  if (bind(sockfd, (struct sockaddr *) &serv_addr, sizeof(serv_addr)) < 0) {
    std::cout << "Could not bind socket" << std::endl;
    return 1;
  };

  std::cout << "Listening on port 8006" << std::endl;

  listen(sockfd,5);

  while (true) {
    auto client_len = sizeof(client_addr);
    auto client_fd = accept(sockfd, (struct sockaddr *)&client_addr, &client_len);

    if (client_fd < 0) {
      std::cout << "Could not accept client socket" << std::endl;
      return 1;
    }

    std::cout << "Client Connected!" << std::endl;

    std::thread send_thread([client_fd] {
      while (true) {
        uint8_t len[2];
        auto n_bytes = read(client_fd, len, 2);

        if (n_bytes != 2) {
          std::cout << "Invalid Length!" << std::endl;
          break;
        }

        uint16_t actual_len = *(uint16_t *)len;
        if (actual_len != 18) {
          std::cout << "Unsupported Message Type" << std::endl;
          break;
        }

        uint8_t in_buffer[256];
        auto n = read(client_fd, in_buffer, actual_len);
        if (n != actual_len) {
          // Client Disconnected
          break;
        } else if (in_buffer[17] > 8) {
          std::cout << "Invalid CAN Message Length" << std::endl;
          break;
        } else {
          // Big Endian
          uint32_t id = (((uint32_t)in_buffer[5]) << 24) | (((uint32_t)in_buffer[6]) << 16) | (((uint32_t)in_buffer[7]) << 8) | ((uint32_t)in_buffer[8]);

          int32_t status;
          HAL_CAN_SendMessage(id, &in_buffer[9], in_buffer[17], HAL_CAN_SEND_PERIOD_NO_REPEAT, &status);
        }
      }
    });

    while (true) {
      int32_t status;
      // Modelled after GrappleTCPMessage
      uint8_t out_buffer[20];
      *(uint16_t *)&out_buffer[0] = 18;
      out_buffer[2] = 2;

      uint32_t id = 0;

      HAL_CAN_ReceiveMessage(&id, 0x00, &out_buffer[11], &out_buffer[19], (uint32_t *)&out_buffer[3], &status);

      // Big Endian
      out_buffer[7] = (id >> 24) & 0xFF;
      out_buffer[8] = (id >> 16) & 0xFF;
      out_buffer[9] = (id >> 8) & 0xFF;
      out_buffer[10] = id & 0xFF;

      if (status == 0) {
        if (write(client_fd, out_buffer, 20) < 0) {
          // Client Disconnected
          break;
        }
      }

      std::this_thread::sleep_for(1ms);
    }

    send_thread.join();

    std::cout << "Client Disconnected" << std::endl;
  }
}
