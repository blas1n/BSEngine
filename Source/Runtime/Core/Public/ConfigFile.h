#pragma once

#include <string>
#include <unordered_map>

class ConfigFile final
{
    class ConfigSection final
    {
    public:
        ConfigSection(const std::unordered_map<std::string, std::string>& inData)
            : data(inData) {}

        ConfigSection(std::unordered_map<std::string, std::string>&& inData)
            : data(std::move(inData)) {}
        
        const std::string* operator[](const std::string& key) const noexcept;

    private:
        std::unordered_map<std::string, std::string> data;
    };

public:
    ConfigFile() = default;

    ConfigFile(const std::string& fileName)
        : ConfigFile()
    {
        LoadFromFile(fileName);
    }

    bool LoadFromFile(const std::string& fileName) noexcept;
    void Clear() noexcept;

    bool IsAvailable() const noexcept { return state == State::Available; }
    operator bool() const noexcept { return IsAvailable(); }

    bool IsExistSection(const std::string& section) const noexcept
    {
        return data.find(section) != data.cend();
    }

    const std::string* operator()(const std::string& sectionName, const std::string& keyName) const noexcept;

private:
    enum class State
    {
        Uninitialized,
        Available,
        Error
    } state = State::Uninitialized;

    std::unordered_map<std::string, ConfigSection> data;
};