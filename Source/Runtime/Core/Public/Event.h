#pragma once

#include <vector>
#include "Delegate.h"

template <class T>
class Event
{
	static_assert(sizeof(T) == 0, "Expected a function signature for the event template parameter");
};

template <class R, class... Args>
class Event<R(Args...)> final
{
public:
	using Func = R(*)(Args...);

public:
	Event() noexcept = default;
	Event(std::nullptr_t) noexcept : Event() {}

	Event(const Event&) noexcept = default;
	Event(Event&&) noexcept = default;

	Event& operator=(const Event&) noexcept = default;
	Event& operator=(Event&&) noexcept = default;

	~Event() = default;

	Event& operator+=(Delegate<Func>&& fn)
	{
		funcs.emplace_back(std::forward<Delegate<Func>(fn));
		return *this;
	}

	void operator()(Args&&... args) const
	{
		for (auto& fn : funcs)
			fn(args);
	}

	template <class Fn>
	R operator()(Fn&& mixer, Args&&... args) const
	{
		if (!IsBound()) return R();

		R ret = funcs[0](args...);

		const size_t size = Size();
		for (size_t i = 1; i < size; ++i)
			ret = mixer(ret, fn(args));

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
	std::vector<Delegate<Func>> funcs;
};
