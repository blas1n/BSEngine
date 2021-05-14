#pragma once

template <class T>
class Accessor
{
public:
	static void SetManager(T* inManager) noexcept { manager = inManager; }

protected:
	[[nodiscard]] static T* GetManager() noexcept { return manager; }

private:
	inline static T* manager = nullptr;
};
