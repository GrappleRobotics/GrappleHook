#include "grpl/CanBridge.h"
#include "hal/HAL.h"
#include <iostream>
#include <thread>

int main() {
  if (HAL_Initialize(500, 0) == 0) {
    std::cout << "Failed to Initialise the HAL" << std::endl;
    return 1;
  }

  grpl::start_can_bridge();

  while (true) {
    std::this_thread::sleep_for(std::chrono::seconds(1));
  }
}
