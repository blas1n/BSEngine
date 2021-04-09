#pragma once

#include "DelegateBase.h"
#include "DelegateInst.h"

template <class T>
class Delegate
{
	static_assert(sizeof(T) == 0, "Expected a function signature for the delegate template parameter");
};

template <class R, class... Args>
class Delegate<R(Args...)> final : public Impl::DelegateBase
{
public:
	using Func = R(*)(Args...);

public:
	using DelegateBase::DelegateBase;
	~Delegate() = default;

	Delegate(Func fn)
	{
		Allocate(FunctionDelegateInst{ fn });
	}

	template <class T>
	Delegate(T&& obj, R(T::*fn)(Args...))
	{
		Allocate(MethodDelegateInst{ obj, fn });
	}

	template <class Fn>
	Delegate(Fn&& fn)
	{
		Allocate(FunctorDelegateInst<Func>{ fn });
	}

	R operator()(Args&&... args) const
	{
		if (const void* ptr = GetPtr())
		{
			return reinterpret_cast<DelegateInstBase
				<Func>*>(ptr)(std::forward<Args>(args)...);
		}

		return R();
	}
};
