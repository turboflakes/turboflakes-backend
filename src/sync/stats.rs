// The MIT License (MIT)
// Copyright © 2021 Aukbit Ltd.
// 
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
// 
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
// 
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

pub fn mean(list: &Vec<u32>) -> f64 {
  let sum: u32 = list.iter().sum();
  f64::from(sum) / (list.len() as f64)
}

pub fn median(list: &mut Vec<u32>) -> u32 {
  if list.len() == 0 {
    return 0
  }
  list.sort();
  let mid = list.len() / 2;
  list[mid]
}

pub fn min(list: &Vec<u32>) -> u32 {
  match list.iter().min() {
    Some(v) => *v,
    None => 0,
  }
}

pub fn max(list: &Vec<u32>) -> u32 {
  match list.iter().max() {
    Some(v) => *v,
    None => 0,
  }
}