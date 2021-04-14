#include "gtest/gtest.h"
#include "Core.h"

TEST(CoreTest, AssertTest)
{
	const int32 lhs = 1, rhs = 1;

	Check(lhs == rhs);
	CheckMsg(lhs == rhs, u"{} and {} are different.", lhs, rhs);

	EXPECT_TRUE(Ensure(lhs == rhs));
	EXPECT_TRUE(EnsureMsg(lhs == rhs, u"{} and {} are different.", lhs, rhs));
}

bool TestA(int32 lhs, int32 rhs)
{
	return true;
}

struct Foo final
{
	bool Boo(int32 lhs, int32 rhs)
	{
		return true;
	}
};

TEST(CoreTest, DelegateTest)
{
	auto lambda = [](int32 lhs, int32 rhs)
	{
		return lhs == rhs;
	};

	Foo foo;
	Delegate<bool, int32, int32> delegate;

	delegate = &TestA;
	EXPECT_TRUE(delegate(1, 1));

	delegate = { &foo, &Foo::Boo };
	EXPECT_TRUE(delegate(1, 1));

	delegate = lambda;
	EXPECT_TRUE(delegate(1, 1));
}

static bool ReturnTrue(int32 lhs, int32 rhs)
{
	return true;
}

TEST(CoreTest, EventTest)
{
	Event<bool, int32, int32> event;
	event += [](int32 lhs, int32 rhs)
	{
		return lhs == rhs;
	};

	event(1, 1);
	EXPECT_TRUE(event([](bool result, bool newResult)
		{
			return result && newResult;
		}, 1, 1));
}
