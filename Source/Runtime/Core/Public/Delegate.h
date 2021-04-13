#pragma once

#include <utility>
#include "DelegateInst.h"

template <class R, class... Args>
class Delegate final
{
public:
	Delegate() noexcept : storage() {}

	Delegate(std::nullptr_t) noexcept : Delegate() {}

	Delegate(const Delegate& other) noexcept
	{
		if (const auto* inst = other.GetInst())
			inst->CloneTo(&storage);
	}

	Delegate(Delegate&& other) noexcept
	{
		storage[0] = std::move(other.storage[0]);
		storage[1] = std::move(other.storage[1]);
		other.storage[0] = other.storage[1] = nullptr;
	}

	Delegate& operator=(const Delegate& other) noexcept
	{
		if (*this == other) return *this;

		Clear();

		if (const auto* inst = other.GetInst())
			inst->CloneTo(&storage);

		return *this;
	}

	Delegate& operator=(Delegate&& other) noexcept
	{
		if (*this == other) return *this;

		Clear();
		storage[0] = std::move(other.storage[0]);
		storage[1] = std::move(other.storage[1]);
		other.storage[0] = other.storage[1] = nullptr;

		return *this;
	}

	Delegate(R(*fn)(Args...))
	{
		Impl::DelegateInstFunction<R, Args...>{ fn }.MoveTo(storage);
	}

	template <class T>
	Delegate(T* obj, R(T::* fn)(Args...))
	{
		Impl::DelegateInstMethod<T, R, Args...>{ obj, fn }.MoveTo(storage);
	}

	template <class T>
	Delegate(T* obj, R(T::* fn)(Args...) const)
	{
		Impl::DelegateInstConstMethod<R, Args...>{ obj, fn }.MoveTo(storage);
	}

	template <class Func>
	Delegate(Func&& fn)
	{
		Impl::DelegateInstFunctor<Func, R, Args...>{ std::forward<Func>(fn) }.MoveTo(storage);
	}

	~Delegate() { Clear(); }

	R operator()(const Args&... args)
	{
		return IsBound() ? GetInst()->Execute(args...) : R();
	}

	R operator()(Args&&... args)
	{
		return IsBound() ? GetInst()->Execute(std::move(args)...) : R();
	}

	void Clear() noexcept
	{
		if (const auto heap = GetHeap())
			delete heap;
		
		storage[0] = storage[1] = nullptr;
	}

	[[nodiscard]] bool IsBound() const noexcept { return storage[0]; }
	[[nodiscard]] operator bool() const noexcept { return IsBound(); }

private:
	[[nodiscard]] decltype(auto) GetInst() noexcept
	{
		if (const auto heap = GetHeap())
			return reinterpret_cast<Impl::DelegateInstBase<R, Args...>*>(heap);
		
		return reinterpret_cast<Impl::DelegateInstBase<R, Args...>*>(&storage);
	}

	[[nodiscard]] void* GetHeap() noexcept
	{
		return (storage[0] && !storage[1]) ? storage[0] : nullptr;
	}

private:
	void* storage[2];
};

template <class R, class... Args>
[[nodiscard]] bool operator==(const Delegate<R, Args...>& lhs, const Delegate<R, Args...>& rhs)
{
	return &lhs == &rhs;
}

template <class R, class... Args>
[[nodiscard]] bool operator!=(const Delegate<R, Args...>& lhs, const Delegate<R, Args...>& rhs)
{
	return !(lhs == rhs);
}
