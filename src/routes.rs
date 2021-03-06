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

use crate::handlers::{
    era::get_era,
    health::get_health,
    info::get_info,
    validator::{get_validator, get_validator_eras, get_validator_rank, get_validators},
};
use actix_web::web;

/// All routes are placed here
pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg
        // Index
        .route("/", web::get().to(get_info))
        // Healthcheck
        .route("/health", web::get().to(get_health))
        // /api/v1 routes
        .service(
            web::scope("/api/v1")
                // API info
                .route("", web::get().to(get_info))
                // ERA routes
                .service(web::scope("/era").route("/{era_index}", web::get().to(get_era)))
                // VALIDATOR routes
                .service(
                    web::scope("/validator")
                        .route("/{stash}", web::get().to(get_validator))
                        .route("/{stash}/rank", web::get().to(get_validator_rank))
                        .route("/{stash}/eras", web::get().to(get_validator_eras))
                        .route("", web::get().to(get_validators)),
                ),
        );
}
