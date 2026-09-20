// tests/fixtures/sample_workspace/cpp_app/engine.h
#pragma once
#include <string>

class BaseEngine {
public:
    virtual ~BaseEngine() = default;
    virtual void run() = 0;
    virtual std::string get_name() const {
        return "BaseEngine";
    }
};

class AdvancedEngine : public BaseEngine {
private:
    int power_level;
public:
    AdvancedEngine(int power = 100) : power_level(power) {}
    void run() override {}
    int get_power() const { return power_level; }
};
