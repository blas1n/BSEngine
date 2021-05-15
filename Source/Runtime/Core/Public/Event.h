#pragma once

#include <vector>
#include "Delegate.h"

template <class T>
class Event;

template <class R, class... Args>
class Event<R(Args...)> final
{
public:
	Event() noexcept = default;
	Event(std::nullptr_t) noexcept : Event() {}

	Event(const Event&) noexcept = default;
	Event(Event&&) noexcept = default;

	Event& operator=(const Event&) noexcept = default;
	Event& operator=(Event&&) noexcept = default;

	~Event() = default;

	Event& operator+=(const Delegate<R(Args...)>& fn)
	{
		funcs.emplace_back(fn);
		return *this;
	}

	Event& operator+=(Delegate<R(Args...)>&& fn)
	{
		funcs.emplace_back(std::move(fn));
		return *this;
	}

	Event& operator-=(const Delegate<R(Args...)>& fn)
	{
		funcs.erase(std::find(funcs.cbegin(), funcs.cend(), fn));
		return *this;
	}

	void operator()(Args... args) const
	{
		for (auto& fn : funcs)
			fn(args...);
	}

	template <class Fn>
	R operator()(Fn&& mixer, const Args&... args)
	{
		if (!IsBound()) return R();

		R ret = funcs[0](args...);

		const size_t size = Size();
		for (size_t i = 1; i < size; ++i)
			ret = mixer(ret, funcs[i](args...));

		return ret;
	}

	[[nodiscard]] operator bool() const noexcept
	{
		return !funcs.empty();
	}

	[[nodiscard]] bool IsBound() const noexcept
	{
		return !funcs.empty();
	}

	size_t Size() const noexcept
	{
		return funcs.size();
	}

	void Clear() noexcept
	{
		for (auto& fn : funcs)
			fn.Clear();

		funcs.clear();
	}

	friend bool operator==(const Event& lhs, const Event& rhs);

private:
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
	std::vector<Delegate<R(Args...)>> funcs;
};

template <class T>
[[nodiscard]] bool operator==(const Event<T>& lhs, const Event<T>& rhs)
{
	const size_t size = lhs.Size();
	if (size != rhs.Size())
		return false;

	for (size_t i = 0; i < size; ++i)
		if (lhs.funcs[i] != rhs.funcs[i])
			return false;

	return true;
}

template <class T>
[[nodiscard]] bool operator!=(const Event<T>& lhs, const Event<T>& rhs)
{
	return !(lhs == rhs);
}
