// tests/fixtures/sample_workspace/cpp_app/main.cpp
#include "engine.h"
#include <iostream>

int main(int argc, char* argv[]) {
    AdvancedEngine engine(250);
    engine.run();
    std::cout << "Engine power: " << engine.get_power() << std::endl;
    return 0;
}
