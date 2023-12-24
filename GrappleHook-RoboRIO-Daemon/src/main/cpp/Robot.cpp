#include "grpl/CanBridge.h"
#include "hal/HAL.h"
#include <iostream>

int main() {
  if (HAL_Initialize(500, 0) == 0) {
    std::cout << "Failed to Initialise the HAL" << std::endl;
    return 1;
  }

  grpl::start_can_bridge();
}
