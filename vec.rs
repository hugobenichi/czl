struct Vec {
  x: i32,
  y: i32,
}

impl Vec {
  fn add(&self, v: Vec) -> Vec {
    return vec(self.x + v.x, self.y + v.y);
  }
}

fn vec(x: i32, y: i32) -> Vec {
  return Vec {
    x,
    y,
  };
}

fn main() {
  let v1 = vec(4,5);
  let v2 = vec(1,1);

  let v = v1.add(v2);

  println!("hello {} {}", v.x, v.y);
}
