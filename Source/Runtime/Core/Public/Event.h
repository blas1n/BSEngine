#pragma once

#include <functional>
#include <vector>

template <class R, class... Args>
class Event final
{
public:
	Event() noexcept = default;
	Event(std::nullptr_t) noexcept : Event() {}

	Event(const Event&) = default;
	Event(Event&&) noexcept = default;

	Event& operator=(const Event&) = default;
	Event& operator=(Event&&) noexcept = default;

	~Event() = default;

	Event& operator=(std::nullptr_t)
	{
		funcs.clear();
		return *this;
	}

	template <class Fn>
	Event& operator+=(Fn&& fn)
	{
		funcs.emplace_back(std::forward<Fn>(fn));
		return *this;
	}

	template <class T>
	Event& operator+=(T&& obj, R(T::*fn)(Args...))
	{
		funcs.emplace_back([obj, fn](Args&&... args) { return obj.*fn(std::forward<Args>(args)...) });
		return *this;
	}

	void operator()(Args&&... args)
	{
		for (auto& fn : funcs)
			fn(std::forward<Args>(args)...);
	}

	[[nodiscard]] operator bool() const noexcept
	{
		return !funcs.empty();
	}

	[[nodiscard]] bool IsBound() const noexcept
	{
		return !funcs.empty();
	}

	void Clear() noexcept
	{
		funcs.clear();
	}

	[[nodiscard]] decltype(auto) begin() noexcept
	{
		return funcs.begin();
	}

	[[nodiscard]] decltype(auto) begin() const noexcept
	{
		return funcs.begin();
	}

	[[nodiscard]] decltype(auto) end() noexcept
	{
		return funcs.end();
	}

	[[nodiscard]] decltype(auto) end() const noexcept
	{
		return funcs.end();
	}

private:
	std::vector<std::function<R(Args...)>> funcs;
};