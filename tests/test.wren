
class Test {
  static assert(cond) {
    assert(cond, "Assertion error")
  }

  static assert(cond, msg) {
    if (!cond) {
      Fiber.abort(msg)
    }
  }

  static assertEq(lhs, rhs, msg) {
    if (lhs != rhs) {
      Fiber.abort("Unexpected value. %(msg) %(lhs) != %(rhs)")
    }
  }

  static shouldFail(msg, fn) {
    var fiber = Fiber.new(fn)
    fiber.try()
    if (!fiber.error) {
      Fiber.abort("Should have failed, but didn't. %(msg)")
    }
    return fiber.error
  }

  static shouldFailWith(msg, errorMsg, fn) {
    var error = shouldFail(msg, fn)
    if (error != errorMsg) {
      Fiber.abort("Unexpected error message. Expected '%(errorMsg)' Actual '%(error)'")
    }
  }
}
