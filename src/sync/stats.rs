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
#![allow(dead_code)]

pub fn mean(list: &Vec<u32>) -> f64 {
    if list.len() == 0 {
        return 0.0;
    }
    let sum: u32 = list.iter().sum();
    f64::from(sum) / (list.len() as f64)
}

pub fn mean_f64(list: &Vec<f64>) -> f64 {
    if list.len() == 0 {
        return 0.0;
    }
    let sum: f64 = list.iter().sum();
    sum / (list.len() as f64)
}

pub fn _mean_u128(list: &Vec<u128>) -> f64 {
    if list.len() == 0 {
        return 0.0;
    }
    let sum: u128 = list.iter().sum();
    (sum as f64) / (list.len() as f64)
}

pub fn median(list: &mut Vec<u32>) -> u32 {
    if list.len() == 0 {
        return 0;
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

pub fn standard_deviation(list: &Vec<f64>) -> f64 {
    let m = mean_f64(list);
    let mut variance: Vec<f64> = list
        .iter()
        .map(|&score| (score as f64 - m).powf(2.0))
        .collect();
    mean_f64(&mut variance).sqrt()
}

// Calculate 95% confidence interval
// https://www.mathsisfun.com/data/confidence-interval.html
pub fn confidence_interval_95(list: &Vec<f64>) -> (f64, f64) {
    let m = mean_f64(list);
    let sd = standard_deviation(list);
    let v = 1.96 * (sd / ((list.len() as f64).sqrt()));
    (m - v, m + v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculate_mean() {
        let v = vec![1, 2, 3, 4, 5, 4, 2, 6];
        assert_eq!(mean(&v), 3.375);
    }

    #[test]
    fn calculate_mean_f64() {
        let v = vec![1.2, 2.3, 3.5, 4.0, 5.1, 4.2, 2.7, 6.3];
        assert_eq!(mean_f64(&v), 3.6625);
    }

    #[test]
    fn calculate_median() {
        let mut v = vec![1, 2, 3, 4, 5, 4, 2, 6];
        assert_eq!(median(&mut v), 4);
    }

    #[test]
    fn calculate_min() {
        let mut v = vec![1, 2, 3, 4, 5, 4, 2, 6];
        assert_eq!(min(&mut v), 1);
    }

    #[test]
    fn calculate_max() {
        let mut v = vec![1, 2, 3, 4, 5, 4, 2, 6];
        assert_eq!(max(&mut v), 6);
    }

    #[test]
    fn calculate_standard_deviation() {
        let mut v = vec![600.0, 470.0, 170.0, 430.0, 300.0];
        assert_eq!(standard_deviation(&mut v), 147.32277488562318);
    }
    #[test]
    fn calculate_confidence_interval_95() {
        let mut v = vec![600.0, 470.0, 170.0, 430.0, 300.0];
        assert_eq!(
            confidence_interval_95(&mut v),
            (264.86589420296434, 523.1341057970357)
        );
    }
}
