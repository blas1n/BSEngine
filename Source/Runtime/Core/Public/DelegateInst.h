#pragma once

template <class T>
struct DelegateInstBase;

template <class R, class... Args>
struct DelegateInstBase<R(Args...)>
{
	virtual ~DelegateInstBase() = default;

	virtual R operator()(Args...) const = 0;
};

template <class T>
struct FunctionDelegateInst;

template <class R, class... Args>
struct FunctionDelegateInst<R(Args...)> final : public DelegateInstBase<R(Args...)>
{
	using Func = R(*)(Args...);

public:
	FunctionDelegateInst(Func inFunc)
		: func(inFunc) {}

	R operator()(Args... args) const override
	{
		if (func)
			return func(args...);
		
		return R();
	}

private:
	Func func;
};

template <class User, class T>
struct MethodDelegateInst;

template <class User, class R, class... Args>
struct MethodDelegateInst<User, R(Args...)> final : public DelegateInstBase<R(Args...)>
{
	using Func = R(User::*)(Args...);

public:
	MethodDelegateInst(User* inUser, Func inFunc)
		: user(inUser), func(inFunc) {}

	R operator()(Args... args) const override
	{
		if (user && func)
			return (user.*func)(args...);

		return R();
	}

private:
	User* user;
	Func func;
};

template <class T, class Func>
struct FunctorDelegateInst;

template <class R, class... Args, class Func>
struct FunctorDelegateInst<R(Args...), Func> final : public DelegateInstBase<R(Args...)>
{
public:
	FunctorDelegateInst(const Func& inFunc)
		: func(inFunc) {}

	FunctorDelegateInst(Func&& inFunc)
		: func(std::move(inFunc)) {}

	R operator()(Args... args) const override
	{
		return func(args...);
	}

private:
	Func func;
};
