#pragma once

#include <vector>
#include "Delegate.h"

template <class R, class... Args>
class Event final
{
public:
	Event() noexcept = default;
	Event(std::nullptr_t) noexcept : Event() {}

	Event(const Event&) noexcept = default;
	Event(Event&&) noexcept = default;

	Event& operator=(const Event&) noexcept = default;
	Event& operator=(Event&&) noexcept = default;

	~Event() = default;

	Event& operator+=(Delegate<R, Args...>&& fn)
	{
		funcs.emplace_back(std::forward<Delegate<R, Args...>>(fn));
		return *this;
	}

	void operator()(const Args&... args)
	{
		for (auto& fn : funcs)
			fn(args...);
	}

	template <class Fn>
	R operator()(Fn&& mixer, Args&&... args) const
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
	std::vector<Delegate<R, Args...>> funcs;
};
